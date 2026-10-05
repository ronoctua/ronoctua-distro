//! The tree into a value.
//!
//! # Units are carried, not converted
//!
//! `10 km / 2 h` is `5 km/h`, written in the units the person wrote; only an
//! addition converts (its right side into the left side's units) and only `in`
//! chooses. A product whose dimensions cancel (`10 km / 5 m`) becomes a bare
//! number. A conversion is exact unless it has to go through π (a degree into
//! a radian) or a transcendental touched the number.
//!
//! # Three kinds of unit convert differently
//!
//! An AFFINE one (°C, °F) adds its offset — but only in a conversion of a
//! temperature, never in an addition: `20 °C + 5 K` is 25 °C, because the 5
//! is a difference. A RECIPROCAL one (mpg, km/L) converts by division against
//! L/100 km. Every other one multiplies.

use std::path::Path;

use num_traits::{ToPrimitive as _, Zero as _};
use ronoctua_files::when::{self, Zone};

use crate::num::{self, Rounding, ratio};
use crate::parse::{Node, NodeKind, Target};
use crate::rates::Currency;
use crate::units::{self, Category, Constant, Function, Radix};
use crate::{Base, BigRational, Env, Number, Quantity, Refusal, Shown, Value, Why};

/// A quantity as it is being computed.
#[derive(Debug, Clone, PartialEq)]
struct Q {
    n: Number,
    unit: Vec<(Base, i8)>,
    /// A percentage (`10%`): `150 + 10%` is 165, and `10% + 5%` is shown as
    /// a percentage.
    percent: bool,
    shown: Shown,
}

impl Q {
    fn bare(n: Number) -> Self {
        Self {
            n,
            unit: Vec::new(),
            percent: false,
            shown: Shown::Decimal,
        }
    }

    fn of(base: Base) -> Self {
        Self {
            unit: vec![(base, 1)],
            ..Self::bare(Number::one())
        }
    }
}

/// An instant: seconds since the epoch, whether it is a whole day, and the
/// offset it is shown in when `in` chose a zone.
#[derive(Debug, Clone, PartialEq)]
struct At {
    secs: i64,
    date_only: bool,
    offset: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
enum Val {
    Q(Q),
    At(At),
}

pub fn value(node: &Node, env: &Env) -> Result<Value, Refusal> {
    let v = eval(node, env)?;
    Ok(match v {
        Val::Q(q) => Value {
            number: q.n,
            unit: (!q.unit.is_empty()).then_some(Quantity { terms: q.unit }),
            shown: if q.percent { Shown::Percent } else { q.shown },
        },
        Val::At(at) => {
            let offset = at.offset.unwrap_or_else(|| env.zone.offset_at(at.secs));
            Value {
                number: Number::Exact(BigRational::from_integer(at.secs.into())),
                unit: None,
                shown: if at.date_only {
                    Shown::Date { offset_s: offset }
                } else {
                    Shown::DateTime { offset_s: offset }
                },
            }
        }
    })
}

fn eval(node: &Node, env: &Env) -> Result<Val, Refusal> {
    let refuse = |why: Why| Refusal { at: node.at, why };
    let q = |n: &Node| -> Result<Q, Refusal> {
        match eval(n, env)? {
            Val::Q(q) => Ok(q),
            Val::At(_) => Err(Refusal {
                at: n.at,
                why: Why::Incompatible,
            }),
        }
    };
    Ok(match &node.kind {
        NodeKind::Number(n, _) => Val::Q(Q::bare(Number::Exact(n.clone()))),
        NodeKind::Roman(n) => Val::Q(Q::bare(num::exact(i64::from(*n), 1))),
        NodeKind::Unit(u) => Val::Q(Q::of(Base::Unit(*u))),
        NodeKind::Currency(c) => Val::Q(Q::of(Base::Currency(*c))),
        NodeKind::Constant(c) => Val::Q(Q::bare(Number::Approx(match c {
            Constant::Pi => std::f64::consts::PI,
            Constant::E => std::f64::consts::E,
            Constant::Tau => std::f64::consts::TAU,
        }))),
        NodeKind::Negate(inner) => {
            let mut v = q(inner)?;
            v.n = v.n.neg();
            Val::Q(v)
        }
        NodeKind::Add(l, r) | NodeKind::Subtract(l, r) => {
            let minus = matches!(node.kind, NodeKind::Subtract(..));
            match (eval(l, env)?, eval(r, env)?) {
                (Val::Q(a), Val::Q(b)) => Val::Q(add(a, b, minus, env).map_err(refuse)?),
                (Val::At(a), Val::Q(b)) => Val::At(shift(a, &b, minus, env).map_err(refuse)?),
                (Val::Q(a), Val::At(b)) if !minus => {
                    Val::At(shift(b, &a, false, env).map_err(refuse)?)
                }
                (Val::At(a), Val::At(b)) if minus => Val::Q(between(&a, &b).map_err(refuse)?),
                _ => return Err(refuse(Why::Incompatible)),
            }
        }
        NodeKind::Multiply(l, r) | NodeKind::Divide(l, r) => {
            let (a, b) = (q(l)?, q(r)?);
            let divide = matches!(node.kind, NodeKind::Divide(..));
            let n = if divide { a.n.div(&b.n) } else { a.n.mul(&b.n) }.map_err(refuse)?;
            let unit = merge(&a.unit, &b.unit, if divide { -1 } else { 1 }).map_err(refuse)?;
            Val::Q(
                collapse(
                    Q {
                        n,
                        unit,
                        percent: false,
                        shown: Shown::Decimal,
                    },
                    env,
                )
                .map_err(refuse)?,
            )
        }
        NodeKind::Power(b, e) => {
            let (base, exponent) = (q(b)?, q(e)?);
            if !exponent.unit.is_empty() {
                return Err(refuse(Why::Incompatible));
            }
            let unit = if base.unit.is_empty() {
                Vec::new()
            } else {
                let k = exponent
                    .n
                    .integer()
                    .and_then(|k| k.to_i8())
                    .ok_or(refuse(Why::Domain))?;
                base.unit
                    .iter()
                    .map(|(u, p)| p.checked_mul(k).map(|p| (*u, p)).ok_or(Why::TooLarge))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(refuse)?
            };
            let n = base.n.pow(&exponent.n).map_err(refuse)?;
            Val::Q(Q {
                n,
                unit: unit.into_iter().filter(|(_, p)| *p != 0).collect(),
                percent: false,
                shown: Shown::Decimal,
            })
        }
        NodeKind::Percent(inner) => {
            let v = q(inner)?;
            if !v.unit.is_empty() {
                return Err(refuse(Why::Incompatible));
            }
            Val::Q(Q {
                n: v.n.div(&num::exact(100, 1)).map_err(refuse)?,
                percent: true,
                ..Q::bare(Number::zero())
            })
        }
        NodeKind::Factorial(inner) => {
            let v = q(inner)?;
            if !v.unit.is_empty() {
                return Err(refuse(Why::Incompatible));
            }
            Val::Q(Q::bare(v.n.factorial().map_err(refuse)?))
        }
        NodeKind::Call(f, argument) => Val::Q(call(*f, q(argument)?, env).map_err(refuse)?),
        NodeKind::Date(y, m, d) => Val::At(At {
            secs: local(*y, *m, *d, 0, &env.zone).ok_or(refuse(Why::BadDate))?,
            date_only: true,
            offset: None,
        }),
        NodeKind::Time(h, m, s) => {
            let today = today(env);
            Val::At(At {
                secs: today + i64::from(h * 3600 + m * 60 + s),
                date_only: false,
                offset: None,
            })
        }
        NodeKind::Now => Val::At(At {
            secs: env.now_unix_s,
            date_only: false,
            offset: None,
        }),
        NodeKind::Today => Val::At(At {
            secs: today(env),
            date_only: true,
            offset: None,
        }),
        NodeKind::Convert(inner, target) => convert(eval(inner, env)?, target, env, node)?,
    })
}

fn convert(v: Val, target: &Target, env: &Env, node: &Node) -> Result<Val, Refusal> {
    let refuse = |why: Why| Refusal { at: node.at, why };
    match (v, target) {
        (Val::Q(v), Target::Units(t)) => {
            let Val::Q(t) = eval(t, env)? else {
                return Err(refuse(Why::Incompatible));
            };
            if t.unit.is_empty() || t.percent {
                return Err(refuse(Why::Incompatible));
            }
            let n = into(&v, &t.unit, true, env)
                .and_then(|n| n.div(&t.n))
                .map_err(refuse)?;
            Ok(Val::Q(Q {
                n,
                unit: t.unit,
                percent: false,
                shown: Shown::Decimal,
            }))
        }
        (Val::Q(mut v), Target::Radix(r)) => {
            if !v.unit.is_empty() {
                return Err(refuse(Why::Incompatible));
            }
            let integer = v.n.integer().ok_or(refuse(Why::Domain))?;
            if *r == Radix::Roman && crate::lex::roman_text(integer.to_u32().unwrap_or(0)).is_none()
            {
                return Err(refuse(Why::BadRoman));
            }
            v.shown = Shown::Radix(*r);
            v.percent = false;
            Ok(Val::Q(v))
        }
        (Val::At(mut at), Target::Zone(name)) => {
            let zone = zone_named(name, env).map_err(refuse)?;
            at.offset = Some(zone.offset_at(at.secs));
            at.date_only = false;
            Ok(Val::At(at))
        }
        (Val::Q(_), Target::Zone(_)) => Err(refuse(Why::UnknownWord)),
        _ => Err(refuse(Why::Incompatible)),
    }
}

/// `a + b` (or `a − b`): `b` in `a`'s units, a percentage as a share of `a`.
fn add(a: Q, b: Q, minus: bool, env: &Env) -> Result<Q, Why> {
    if b.percent && !a.percent {
        // `150 + 10%` is 165: the percentage is of the left side.
        let share = if minus {
            Number::one().sub(&b.n)?
        } else {
            Number::one().add(&b.n)?
        };
        return Ok(Q {
            n: a.n.mul(&share)?,
            ..a
        });
    }
    let right = if a.unit == b.unit {
        b.n
    } else {
        into(&b, &a.unit, false, env)?
    };
    let n = if minus {
        a.n.sub(&right)?
    } else {
        a.n.add(&right)?
    };
    Ok(Q {
        n,
        percent: a.percent && b.percent,
        ..a
    })
}

/// Two unit lists multiplied (`sign` 1) or divided (`sign` −1): powers of
/// the same base add, and a base whose power reaches zero leaves.
fn merge(a: &[(Base, i8)], b: &[(Base, i8)], sign: i8) -> Result<Vec<(Base, i8)>, Why> {
    let mut out = a.to_vec();
    for (base, p) in b {
        let p = p.checked_mul(sign).ok_or(Why::TooLarge)?;
        match out.iter_mut().find(|(u, _)| u == base) {
            Some((_, q)) => *q = q.checked_add(p).ok_or(Why::TooLarge)?,
            None => out.push((*base, p)),
        }
    }
    out.retain(|(_, p)| *p != 0);
    Ok(out)
}

/// A quantity whose dimensions cancelled (`km/m`) as a bare number.
fn collapse(q: Q, env: &Env) -> Result<Q, Why> {
    if q.unit.is_empty() {
        return Ok(q);
    }
    let d = dimension(&q.unit);
    if d.powers.iter().any(|p| *p != 0) || d.money != 0 || d.fuel {
        return Ok(q);
    }
    Ok(Q::bare(into(&q, &[], false, env)?))
}

/// What a unit list measures: its powers, its currency power, and whether it
/// is a fuel economy (kept apart from the areas it has the dimension of).
struct Dim {
    powers: [i32; 6],
    money: i32,
    fuel: bool,
}

fn dimension(unit: &[(Base, i8)]) -> Dim {
    let mut d = Dim {
        powers: [0; 6],
        money: 0,
        fuel: false,
    };
    for (base, p) in unit {
        match base {
            Base::Unit(u) => {
                if let Some(u) = units::table().unit(*u) {
                    for (k, x) in u.dimension.0.iter().enumerate() {
                        d.powers[k] += i32::from(*x) * i32::from(*p);
                    }
                    d.fuel |= u.category == Category::FuelEconomy;
                }
            }
            Base::Currency(_) => d.money += i32::from(*p),
        }
    }
    d
}

/// The exact factor of a unit list to its base units, and the power of π
/// that goes with it. Currencies are left out: they go through the rates.
fn factor(unit: &[(Base, i8)]) -> Result<(Number, i32), Why> {
    let mut f = Number::one();
    let mut pi = 0i32;
    for (base, p) in unit {
        if let Base::Unit(u) = base {
            let u = units::table().unit(*u).ok_or(Why::UnknownWord)?;
            f = f.mul(&Number::Exact(ratio(u.factor)).pow(&num::exact(i64::from(*p), 1))?)?;
            pi += i32::from(u.pi) * i32::from(*p);
        }
    }
    Ok((f, pi))
}

/// How many euros one unit of a currency is worth, by the rates.
fn in_euros(c: Currency, env: &Env) -> Result<Number, Why> {
    if c == Currency::EUR {
        return Ok(Number::one());
    }
    let rates = env.rates.as_ref().ok_or(Why::NoRates)?;
    let per = rates.per_euro.get(&c).ok_or(Why::UnknownCurrency)?;
    Number::Exact(per.clone()).recip()
}

/// The single unit of a list that is one unit to the first power.
fn single(unit: &[(Base, i8)]) -> Option<&'static units::Unit> {
    match unit {
        [(Base::Unit(u), 1)] => units::table().unit(*u),
        _ => None,
    }
}

/// `q`'s number in the units `to`. `affine` applies the offsets of a
/// temperature (a conversion); an addition converts a DIFFERENCE, without.
fn into(q: &Q, to: &[(Base, i8)], affine: bool, env: &Env) -> Result<Number, Why> {
    if q.unit == to {
        return Ok(q.n.clone());
    }
    let (from_d, to_d) = (dimension(&q.unit), dimension(to));
    if from_d.powers != to_d.powers || from_d.money != to_d.money || from_d.fuel != to_d.fuel {
        return Err(Why::Incompatible);
    }
    if let (Some(s), Some(t)) = (single(&q.unit), single(to)) {
        let (fs, ft) = (
            Number::Exact(ratio(s.factor)),
            Number::Exact(ratio(t.factor)),
        );
        if affine && (s.is_affine() || t.is_affine()) {
            let (os, ot) = (
                Number::Exact(ratio(s.offset)),
                Number::Exact(ratio(t.offset)),
            );
            return q.n.mul(&fs)?.add(&os)?.sub(&ot)?.div(&ft);
        }
        if s.reciprocal || t.reciprocal {
            let base = if s.reciprocal {
                fs.div(&q.n)?
            } else {
                q.n.mul(&fs)?
            };
            return if t.reciprocal {
                ft.div(&base)
            } else {
                base.div(&ft)
            };
        }
    }
    let (fs, pis) = factor(&q.unit)?;
    let (ft, pit) = factor(to)?;
    let mut n = q.n.mul(&fs)?.div(&ft)?;
    // CURRENCIES by their net power: `USD/USD` needs no rates.
    let mut net: Vec<(Currency, i32)> = Vec::new();
    for (unit, sign) in [(&q.unit[..], 1), (to, -1)] {
        for (base, p) in unit {
            if let Base::Currency(c) = base {
                match net.iter_mut().find(|(x, _)| x == c) {
                    Some((_, k)) => *k += sign * i32::from(*p),
                    None => net.push((*c, sign * i32::from(*p))),
                }
            }
        }
    }
    for (c, k) in net.into_iter().filter(|(_, k)| *k != 0) {
        n = n.mul(&in_euros(c, env)?.pow(&num::exact(i64::from(k), 1))?)?;
    }
    let pi = pis - pit;
    if pi != 0 {
        n = n.mul(&num::approx(std::f64::consts::PI.powi(pi))?)?;
    }
    Ok(n)
}

fn call(f: Function, x: Q, env: &Env) -> Result<Q, Why> {
    let keep = |n: Number| Q {
        n,
        unit: x.unit.clone(),
        percent: x.percent,
        shown: Shown::Decimal,
    };
    let root = |k: i8, n: Number| -> Result<Q, Why> {
        let unit = x
            .unit
            .iter()
            .map(|(u, p)| {
                if p % k == 0 {
                    Ok((*u, p / k))
                } else {
                    Err(Why::Domain)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Q {
            n,
            unit,
            percent: false,
            shown: Shown::Decimal,
        })
    };
    match f {
        Function::Sqrt => return root(2, x.n.sqrt()?),
        Function::Cbrt => return root(3, x.n.cbrt()?),
        Function::Abs => return Ok(keep(x.n.abs())),
        Function::Floor => return Ok(keep(x.n.rounded(Rounding::Floor)?)),
        Function::Ceil => return Ok(keep(x.n.rounded(Rounding::Ceil)?)),
        Function::Round => return Ok(keep(x.n.rounded(Rounding::Nearest)?)),
        _ => {}
    }
    // THE TRIGONOMETRY TAKES AN ANGLE, in any unit, or a bare number of
    // radians; everything else takes a bare number.
    let trig = matches!(f, Function::Sin | Function::Cos | Function::Tan);
    let radian = units::table().by_id("radian").ok_or(Why::UnknownWord)?;
    let a = if x.unit.is_empty() {
        x.n.clone()
    } else if trig {
        into(&x, &[(Base::Unit(radian), 1)], false, env)?
    } else {
        return Err(Why::Incompatible);
    };
    // THE EXACT ANSWERS these functions have: `log(1000)` is 3, not
    // 2.9999999999999996, and `sin 0` is 0.
    let exact = match f {
        Function::Log => a.exact_log(10).map(|k| num::exact(k, 1)),
        Function::Log2 => a.exact_log(2).map(|k| num::exact(k, 1)),
        Function::Ln if a == Number::one() => Some(Number::zero()),
        Function::Exp | Function::Cos | Function::Cosh if a.is_zero() => Some(Number::one()),
        Function::Sin
        | Function::Tan
        | Function::Asin
        | Function::Atan
        | Function::Sinh
        | Function::Tanh
            if a.is_zero() =>
        {
            Some(Number::zero())
        }
        _ => None,
    };
    if let Some(n) = exact {
        return Ok(Q::bare(n));
    }
    let mut v = a.to_f64();
    // AN ANGLE THAT IS A KNOWN MULTIPLE OF π is reduced EXACTLY into one
    // turn, and the multiples of π/12 whose sine, cosine or tangent is
    // rational answer exactly: `cos(90°)` is 0, not 6.1e-17, and `tan(90°)`
    // is refused, not 1.6e16. The rest is computed from the reduced angle,
    // which also keeps `sin(1e9 °)` from losing its digits to the turns.
    if trig && let Some(m) = multiple_of_pi(&x, &a) {
        let two = BigRational::from_integer(2.into());
        let turn = &m - &two * (&m / &two).floor();
        let twelfths = &turn * BigRational::from_integer(12.into());
        if twelfths.is_integer()
            && let Some(k) = twelfths.to_integer().to_u32()
            && let Some(answer) = at_twelfths(f, k)?
        {
            return Ok(Q::bare(answer));
        }
        v = Number::Exact(turn).to_f64() * std::f64::consts::PI;
    }
    let domain = |ok: bool| if ok { Ok(()) } else { Err(Why::Domain) };
    let r = match f {
        Function::Exp => v.exp(),
        Function::Ln => {
            domain(v > 0.0)?;
            v.ln()
        }
        Function::Log => {
            domain(v > 0.0)?;
            v.log10()
        }
        Function::Log2 => {
            domain(v > 0.0)?;
            v.log2()
        }
        Function::Sin => v.sin(),
        Function::Cos => v.cos(),
        Function::Tan => v.tan(),
        Function::Asin => {
            domain((-1.0..=1.0).contains(&v))?;
            v.asin()
        }
        Function::Acos => {
            domain((-1.0..=1.0).contains(&v))?;
            v.acos()
        }
        Function::Atan => v.atan(),
        Function::Sinh => v.sinh(),
        Function::Cosh => v.cosh(),
        Function::Tanh => v.tanh(),
        Function::Sqrt
        | Function::Cbrt
        | Function::Abs
        | Function::Floor
        | Function::Ceil
        | Function::Round => {
            unreachable!("answered above")
        }
    };
    Ok(Q::bare(num::approx(r)?))
}

/// The angle `x` (whose value in radians is `radians`) as an exact multiple
/// of π, when it is one: an exact number of a unit that carries π (a degree,
/// a gradian, a turn), a zero, or an approximation within rounding of a
/// multiple of π/12 — what `pi`, `tau` and their simple products are, since
/// they reach here as floating point. An exact number of radians other than
/// zero is never a rational multiple of π.
fn multiple_of_pi(x: &Q, radians: &Number) -> Option<BigRational> {
    if !x.unit.is_empty()
        && let Number::Exact(n) = &x.n
        && let Ok((Number::Exact(f), 1)) = factor(&x.unit)
    {
        return Some(n * f);
    }
    match radians {
        Number::Exact(r) => r.is_zero().then(BigRational::zero),
        Number::Approx(v) => {
            let twelfths = v / std::f64::consts::PI * 12.0;
            let k = twelfths.round();
            // NOT ZERO (`sin(1e-20)` is 1e-20, not 0), and not so large that a
            // double no longer tells one twelfth from the next.
            let near =
                k != 0.0 && k.abs() < 1e12 && (twelfths - k).abs() <= 1e-9 * k.abs().max(1.0);
            let k = k.to_i64().filter(|_| near)?;
            Some(BigRational::new(k.into(), 12.into()))
        }
    }
}

/// The sine, cosine or tangent of `k` twelfths of π (`k` in one turn, 0..24),
/// when it is rational; `None` when it is irrational (`sin(45°)`), and a
/// refusal where the tangent has no value.
fn at_twelfths(f: Function, k: u32) -> Result<Option<Number>, Why> {
    let sine = |k: u32| match k % 24 {
        0 | 12 => Some(num::exact(0, 1)),
        2 | 10 => Some(num::exact(1, 2)),
        6 => Some(num::exact(1, 1)),
        14 | 22 => Some(num::exact(-1, 2)),
        18 => Some(num::exact(-1, 1)),
        _ => None,
    };
    Ok(match f {
        Function::Sin => sine(k),
        Function::Cos => sine(k + 6),
        Function::Tan => match k % 12 {
            6 => return Err(Why::Domain),
            0 => Some(num::exact(0, 1)),
            3 => Some(num::exact(1, 1)),
            9 => Some(num::exact(-1, 1)),
            _ => None,
        },
        _ => None,
    })
}

// ---- instants ---------------------------------------------------------

/// The farthest an instant may be from 1970, in years either way: a date
/// beyond it is refused as too large, never computed into a wrapped number
/// (the calendar arithmetic multiplies years and days in `i64`).
const MOST_YEARS: i64 = 1_000_000;

/// Seconds of a duration, or the months of one when it is a whole number of
/// calendar months or years — `+ 1 month` is a calendar month on a date.
enum Step {
    Seconds(i64),
    Months(i64),
}

fn step_of(q: &Q, env: &Env) -> Result<Step, Why> {
    let second = units::table().by_id("second").ok_or(Why::UnknownWord)?;
    if let Some(u) = single(&q.unit)
        && let Some(n) = q.n.integer().and_then(|n| n.to_i64())
    {
        match u.id {
            "month" => return Ok(Step::Months(n)),
            "year" => return Ok(Step::Months(n.checked_mul(12).ok_or(Why::TooLarge)?)),
            _ => {}
        }
    }
    let secs = into(q, &[(Base::Unit(second), 1)], false, env)?;
    let whole = match secs.rounded(Rounding::Nearest)? {
        Number::Exact(r) => r.to_integer().to_i64(),
        Number::Approx(f) => f.to_i64(),
    };
    Ok(Step::Seconds(whole.ok_or(Why::TooLarge)?))
}

fn shift(at: At, q: &Q, minus: bool, env: &Env) -> Result<At, Why> {
    let sign: i64 = if minus { -1 } else { 1 };
    let far = Why::TooLarge;
    let secs = match step_of(q, env)? {
        Step::Seconds(s) => {
            let secs = sign
                .checked_mul(s)
                .and_then(|s| at.secs.checked_add(s))
                .ok_or(far)?;
            return Ok(At {
                secs: within_years(secs)?,
                date_only: at.date_only && s % 86_400 == 0,
                ..at
            });
        }
        Step::Months(k) => {
            let offset = at.offset.unwrap_or_else(|| env.zone.offset_at(at.secs));
            let local = at.secs.checked_add(i64::from(offset)).ok_or(far)?;
            let (y, m, d) = civil_from_days(local.div_euclid(86_400));
            let time = local.rem_euclid(86_400);
            let months = y
                .checked_mul(12)
                .and_then(|n| n.checked_add(i64::from(m) - 1))
                .and_then(|n| n.checked_add(sign.checked_mul(k)?))
                .ok_or(far)?;
            let (y, m) = (
                months.div_euclid(12),
                u32::try_from(months.rem_euclid(12) + 1).unwrap_or(1),
            );
            if y.abs() > 1970 + MOST_YEARS {
                return Err(far);
            }
            // THE DAY IS KEPT, or the month's last when it has fewer:
            // `2026-01-31 + 1 month` is the 28th of February.
            let d = d.min(days_in(y, m));
            let days = when::epoch_of(y, m, d).ok_or(Why::BadDate)?;
            within_years(days * 86_400 + time - i64::from(offset))?
        }
    };
    Ok(At { secs, ..at })
}

/// `secs`, when it is within [`MOST_YEARS`] of 1970.
fn within_years(secs: i64) -> Result<i64, Why> {
    const MOST: i64 = MOST_YEARS * 366 * 86_400;
    if (-MOST..=MOST).contains(&secs) {
        Ok(secs)
    } else {
        Err(Why::TooLarge)
    }
}

/// The duration between two instants, in the largest unit it has one of.
fn between(a: &At, b: &At) -> Result<Q, Why> {
    let diff = a.secs.checked_sub(b.secs).ok_or(Why::TooLarge)?;
    let id = if a.date_only && b.date_only {
        "day"
    } else {
        match diff.unsigned_abs() {
            s if s >= 86_400 => "day",
            s if s >= 3600 => "hour",
            s if s >= 60 => "minute",
            _ => "second",
        }
    };
    let unit = units::table().by_id(id).ok_or(Why::UnknownWord)?;
    let f = units::table()
        .unit(unit)
        .map(|u| u.factor)
        .ok_or(Why::UnknownWord)?;
    Ok(Q {
        n: num::exact(diff, 1).div(&Number::Exact(ratio(f)))?,
        unit: vec![(Base::Unit(unit), 1)],
        percent: false,
        shown: Shown::Decimal,
    })
}

fn days_in(y: i64, m: u32) -> u32 {
    let next = if m == 12 {
        when::epoch_of(y + 1, 1, 1)
    } else {
        when::epoch_of(y, m + 1, 1)
    };
    match (next, when::epoch_of(y, m, 1)) {
        (Some(a), Some(b)) => u32::try_from(a - b).unwrap_or(31),
        _ => 31,
    }
}

/// Days since 1970-01-01 into a date (Howard Hinnant's `civil_from_days`,
/// the inverse of `when::epoch_of`).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let y = yoe + era * 400;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The instant a local date and time is, in `zone`; `None` for a date the
/// calendar does not have (`2026-02-30`).
fn local(y: i64, m: u32, d: u32, seconds: i64, zone: &Zone) -> Option<i64> {
    if d > days_in(y, m) {
        return None;
    }
    let guess = when::epoch_of(y, m, d)? * 86_400 + seconds;
    let first = guess - i64::from(zone.offset_at(guess));
    Some(guess - i64::from(zone.offset_at(first)))
}

/// Today at midnight in the person's zone.
fn today(env: &Env) -> i64 {
    let offset = i64::from(env.zone.offset_at(env.now_unix_s));
    let day = (env.now_unix_s + offset).div_euclid(86_400);
    let (y, m, d) = civil_from_days(day);
    local(y, m, d, 0, &env.zone).unwrap_or(day * 86_400 - offset)
}

/// A zone by the name a person typed: `UTC`, a tz name
/// (`America/Sao_Paulo`), or a city (`Tokyo`, `são paulo`, `new york`).
fn zone_named(name: &str, env: &Env) -> Result<Zone, Why> {
    let key = |s: &str| -> String {
        units::fold(s.trim())
            .chars()
            .map(|c| if c == '_' || c == ' ' { ' ' } else { c })
            .collect()
    };
    let wanted = key(name);
    if wanted == "utc" || wanted == "gmt" {
        return Ok(Zone::utc());
    }
    let root = env.zoneinfo.as_deref().ok_or(Why::UnknownZone)?;
    let read = |tz: &str| -> Option<Zone> {
        let path = Path::new(tz);
        if path.is_absolute() || tz.contains("..") {
            return None;
        }
        Some(Zone::read_from(&root.join(path))).filter(|z| !z.is_utc())
    };
    if let Some(z) = read(&name.trim().replace(' ', "_")) {
        return Ok(z);
    }
    let table = std::fs::read_to_string(root.join("zone1970.tab")).map_err(|_| Why::UnknownZone)?;
    let found = table
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split('\t').nth(2))
        .find(|tz| {
            key(tz) == wanted
                || tz
                    .rsplit('/')
                    .next()
                    .is_some_and(|city| key(city) == wanted)
        });
    found.and_then(read).ok_or(Why::UnknownZone)
}

/// A date's parts, for the callers in this crate.
pub fn civil_at(secs: i64, offset_s: i32) -> (i64, u32, u32, u32, u32) {
    let local = secs + i64::from(offset_s);
    let (y, m, d) = civil_from_days(local.div_euclid(86_400));
    let rest = local.rem_euclid(86_400);
    (
        y,
        m,
        d,
        u32::try_from(rest / 3600).unwrap_or(0),
        u32::try_from(rest % 3600 / 60).unwrap_or(0),
    )
}

/// A value in other units — the converter's live value.
pub fn convert_value(v: &Value, to: &[(Base, i8)], env: &Env) -> Result<Value, Why> {
    if to.is_empty() || !matches!(v.shown, Shown::Decimal) {
        return Err(Why::Incompatible);
    }
    let q = Q {
        n: v.number.clone(),
        unit: v.unit.as_ref().map(|u| u.terms.clone()).unwrap_or_default(),
        percent: false,
        shown: Shown::Decimal,
    };
    Ok(Value {
        number: into(&q, to, true, env)?,
        unit: Some(Quantity { terms: to.to_vec() }),
        shown: Shown::Decimal,
    })
}

/// How many alternatives an answer offers.
const ALTERNATIVES: usize = 4;

/// The same value in the other units of its category, the most readable first:
/// the ones whose number is closest to ten (a number with many zeros, before
/// or after the mark, is the least useful way to say a quantity).
pub fn alternatives(v: &Value, env: &Env) -> Vec<Value> {
    let Some(u) = v.unit.as_ref().and_then(|u| single(&u.terms)) else {
        return Vec::new();
    };
    let own = units::table().by_id(u.id);
    let mut out: Vec<(f64, Value)> = units::table()
        .of(u.category)
        .into_iter()
        .filter(|id| Some(*id) != own)
        .filter_map(|id| convert_value(v, &[(Base::Unit(id), 1)], env).ok())
        .filter_map(|c| {
            let f = c.number.to_f64().abs();
            (f.is_finite() && f > 0.0).then(|| ((f.log10() - 1.0).abs(), c))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().take(ALTERNATIVES).map(|(_, c)| c).collect()
}
