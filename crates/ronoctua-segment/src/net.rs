//! A TFLite graph, read once and run many times.
//!
//! # What is supported, and why exactly this
//!
//! The eleven operators `selfie_segmenter.tflite` uses, measured by walking its
//! operator table (see the crate's `Cargo.toml`): `ADD`, `AVERAGE_POOL_2D`,
//! `CONV_2D`, `DEPTHWISE_CONV_2D`, `DEQUANTIZE`, `HARD_SWISH`, `LOGISTIC`,
//! `MUL`, `RELU`, `RESIZE_BILINEAR` and MediaPipe's custom
//! `Convolution2DTransposeBias`. Anything else is a REFUSAL at load, with the
//! operator's name in it — never a silent zero tensor at run time, which would
//! read as "nobody is in the frame".
//!
//! # `DEQUANTIZE` is paid at LOAD
//!
//! Every one of the 110 in the model turns a CONSTANT float16 tensor into
//! float32. Running them per frame would redo the same conversion thirty times
//! a second, so they are evaluated once, their outputs become constants, and
//! the op is dropped from the plan.
//!
//! # Semantics follow TFLite's REFERENCE kernels
//!
//! Padding (`ComputePaddingHeightWidth`: the smaller half before), the
//! half-pixel resize (`ComputeInterpolationValues`: the lower bound clamped at
//! zero, the upper at the edge, the weights from the unclamped position), and
//! the average pool's divisor (in-bounds elements only). The transposed
//! convolution is MediaPipe's `transpose_conv_bias.cc`: no kernel flip, bias
//! added once per output. The test carries MediaPipe's own output for one
//! synthetic frame, which is what makes these sentences checked rather than
//! claimed.

use crate::flat::{self, Table};

/// Why a model could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The bytes are not a FlatBuffer this reader can walk.
    Malformed(&'static str),
    /// An operator outside the eleven.
    Unsupported(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(what) => write!(f, "malformed model: {what}"),
            Self::Unsupported(op) => write!(f, "unsupported operator: {op}"),
        }
    }
}

impl std::error::Error for Refusal {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pad {
    Same,
    Valid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    None,
    Relu,
    Relu6,
}

#[derive(Debug, Clone)]
enum Op {
    Conv {
        x: usize,
        w: usize,
        b: usize,
        y: usize,
        pad: Pad,
        sw: usize,
        sh: usize,
        act: Act,
    },
    Depthwise {
        x: usize,
        w: usize,
        b: usize,
        y: usize,
        pad: Pad,
        sw: usize,
        sh: usize,
        act: Act,
    },
    AvgPool {
        x: usize,
        y: usize,
        pad: Pad,
        sw: usize,
        sh: usize,
        fw: usize,
        fh: usize,
        act: Act,
    },
    Add {
        a: usize,
        b: usize,
        y: usize,
        act: Act,
    },
    Mul {
        a: usize,
        b: usize,
        y: usize,
        act: Act,
    },
    Relu {
        x: usize,
        y: usize,
    },
    HardSwish {
        x: usize,
        y: usize,
    },
    Logistic {
        x: usize,
        y: usize,
    },
    Resize {
        x: usize,
        y: usize,
        align: bool,
        half: bool,
    },
    TransposeBias {
        x: usize,
        w: usize,
        b: usize,
        y: usize,
        pad: Pad,
        sw: usize,
        sh: usize,
    },
    MaxPool {
        x: usize,
        y: usize,
        pad: Pad,
        sw: usize,
        sh: usize,
        fw: usize,
        fh: usize,
        act: Act,
    },
    Prelu {
        x: usize,
        alpha: usize,
        y: usize,
    },
    /// Zero padding; `pads` is `[before, after]` per NHWC axis.
    Pad {
        x: usize,
        y: usize,
        pads: [[usize; 2]; 4],
    },
    /// The data does not move: only the shape changes, and the shape is the
    /// output tensor's.
    Reshape {
        x: usize,
        y: usize,
    },
    /// Along `axis`, counted on the four-dimensional padded shape.
    Concat {
        xs: Vec<usize>,
        y: usize,
        axis: usize,
        act: Act,
    },
    /// A mean over height and width — the only reduction either hand model
    /// asks for.
    MeanSpatial {
        x: usize,
        y: usize,
    },
    FullyConnected {
        x: usize,
        w: usize,
        b: Option<usize>,
        y: usize,
        act: Act,
    },
}

/// A graph ready to run.
#[derive(Debug, Clone)]
pub struct Net {
    shapes: Vec<[usize; 4]>,
    consts: Vec<Option<Vec<f32>>>,
    ops: Vec<Op>,
    input: usize,
    outputs: Vec<usize>,
    threads: usize,
}

/// The per-run buffers, kept between frames so a frame allocates nothing.
#[derive(Debug, Default)]
pub struct Scratch {
    values: Vec<Vec<f32>>,
}

const FLOAT32: u8 = 0;
const FLOAT16: u8 = 1;
const INT32: u8 = 2;

/// IEEE half to single, by hand — the whole conversion is eleven lines, and a
/// dependency for it would be the only one in the crate.
fn half(bits: u16) -> f32 {
    let sign = u32::from(bits >> 15) << 31;
    let exp = u32::from((bits >> 10) & 0x1f);
    let frac = u32::from(bits & 0x3ff);
    let value = match (exp, frac) {
        (0, 0) => sign,
        (0, _) => {
            // Subnormal: renormalise.
            let mut e = 127 - 15 + 1;
            let mut f = frac;
            while f & 0x400 == 0 {
                f <<= 1;
                e -= 1;
            }
            sign | ((e as u32) << 23) | ((f & 0x3ff) << 13)
        }
        (0x1f, _) => sign | 0x7f80_0000 | (frac << 13),
        _ => sign | ((exp + 127 - 15) << 23) | (frac << 13),
    };
    f32::from_bits(value)
}

fn shape4(dims: &[i32]) -> [usize; 4] {
    let mut s = [1usize; 4];
    let take = dims.len().min(4);
    for (i, d) in dims[dims.len() - take..].iter().enumerate() {
        s[4 - take + i] = usize::try_from(*d).unwrap_or(1).max(1);
    }
    s
}

fn act_of(code: u8) -> Result<Act, Refusal> {
    match code {
        0 => Ok(Act::None),
        1 => Ok(Act::Relu),
        3 => Ok(Act::Relu6),
        n => Err(Refusal::Unsupported(format!("fused activation {n}"))),
    }
}

fn pad_of(code: u8) -> Pad {
    if code == 1 { Pad::Valid } else { Pad::Same }
}

fn us(v: i32) -> usize {
    usize::try_from(v).unwrap_or(1).max(1)
}

impl Net {
    /// Reads a `.tflite` file's bytes.
    pub fn read(bytes: &[u8]) -> Result<Self, Refusal> {
        let model = flat::root(bytes).ok_or(Refusal::Malformed("root"))?;
        let codes = model
            .tables(1)
            .ok_or(Refusal::Malformed("operator codes"))?;
        let graphs = model.tables(2).ok_or(Refusal::Malformed("subgraphs"))?;
        let buffers = model.tables(4).ok_or(Refusal::Malformed("buffers"))?;
        let graph = graphs.first().ok_or(Refusal::Malformed("no subgraph"))?;
        let tensors = graph.tables(0).ok_or(Refusal::Malformed("tensors"))?;
        let inputs = graph.ints(1).ok_or(Refusal::Malformed("inputs"))?;
        let outputs = graph.ints(2).ok_or(Refusal::Malformed("outputs"))?;
        let operators = graph.tables(3).ok_or(Refusal::Malformed("operators"))?;

        let mut shapes = Vec::with_capacity(tensors.len());
        let mut ranks = Vec::with_capacity(tensors.len());
        let mut consts: Vec<Option<Vec<f32>>> = Vec::with_capacity(tensors.len());
        let mut halves: Vec<Option<Vec<f32>>> = Vec::with_capacity(tensors.len());
        let mut ints: Vec<Option<Vec<i32>>> = Vec::with_capacity(tensors.len());
        for t in &tensors {
            let dims = t.ints(0).unwrap_or_default();
            shapes.push(shape4(&dims));
            ranks.push(dims.len().clamp(1, 4));
            let kind = t.u8(1, FLOAT32);
            let data = buffers
                .get(t.u32(2, 0) as usize)
                .and_then(|b| b.bytes_of(0))
                .filter(|d| !d.is_empty());
            let (f, h, i) = match (kind, data) {
                (FLOAT32, Some(d)) => (
                    Some(
                        d.as_chunks::<4>()
                            .0
                            .iter()
                            .map(|c| f32::from_le_bytes(*c))
                            .collect(),
                    ),
                    None,
                    None,
                ),
                (FLOAT16, Some(d)) => (
                    None,
                    Some(
                        d.as_chunks::<2>()
                            .0
                            .iter()
                            .map(|c| half(u16::from_le_bytes(*c)))
                            .collect(),
                    ),
                    None,
                ),
                (INT32, Some(d)) => (
                    None,
                    None,
                    Some(
                        d.as_chunks::<4>()
                            .0
                            .iter()
                            .map(|c| i32::from_le_bytes(*c))
                            .collect(),
                    ),
                ),
                _ => (None, None, None),
            };
            consts.push(f);
            halves.push(h);
            ints.push(i);
        }

        let slot = |v: i32| -> Result<usize, Refusal> {
            usize::try_from(v)
                .ok()
                .filter(|i| *i < tensors.len())
                .ok_or(Refusal::Malformed("tensor index"))
        };

        let mut ops = Vec::new();
        let mut packed: Vec<usize> = Vec::new();
        for op in &operators {
            let code = codes
                .get(op.u32(0, 0) as usize)
                .ok_or(Refusal::Malformed("opcode index"))?;
            let old = code.u8(0, 0);
            let builtin = if old == 127 {
                code.i32(3, 0)
            } else {
                i32::from(old)
            };
            let ins = op.ints(1).unwrap_or_default();
            let outs = op.ints(2).unwrap_or_default();
            let i = |n: usize| {
                ins.get(n)
                    .copied()
                    .ok_or(Refusal::Malformed("operator input"))
                    .and_then(slot)
            };
            let y = outs
                .first()
                .copied()
                .ok_or(Refusal::Malformed("operator output"))
                .and_then(slot)?;
            let opts = op.table(4);
            let o = |f: &dyn Fn(Table<'_>) -> Result<Op, Refusal>| -> Result<Op, Refusal> {
                f(opts.ok_or(Refusal::Malformed("builtin options"))?)
            };
            match builtin {
                // DEQUANTIZE: done here, once.
                6 => {
                    let x = i(0)?;
                    let values =
                        halves[x]
                            .clone()
                            .or_else(|| consts[x].clone())
                            .ok_or_else(|| {
                                Refusal::Unsupported("DEQUANTIZE of a live tensor".into())
                            })?;
                    consts[y] = Some(values);
                }
                3 => {
                    // PACKED at load, `[oc][kh][kw][ic]` → `[kh][kw][ic][oc]`:
                    // the inner loop then runs along the output channels, which
                    // is contiguous in both the weights and the accumulator and
                    // is what the compiler vectorises. Measured on the hand
                    // landmark model: see `Net::conv`.
                    let w = i(1)?;
                    if !packed.contains(&w) {
                        let [oc, kh, kw, ic] = shapes[w];
                        if let Some(raw) = consts[w].as_ref() {
                            let mut out = vec![0.0f32; raw.len()];
                            for o in 0..oc {
                                for k in 0..kh * kw {
                                    for c in 0..ic {
                                        out[(k * ic + c) * oc + o] =
                                            raw[(o * kh * kw + k) * ic + c];
                                    }
                                }
                            }
                            consts[w] = Some(out);
                            packed.push(w);
                        } else {
                            return Err(Refusal::Unsupported("CONV_2D by live weights".into()));
                        }
                    }
                    ops.push(o(&|t| {
                        Ok(Op::Conv {
                            x: i(0)?,
                            w: i(1)?,
                            b: i(2)?,
                            y,
                            pad: pad_of(t.u8(0, 0)),
                            sw: us(t.i32(1, 1)),
                            sh: us(t.i32(2, 1)),
                            act: act_of(t.u8(3, 0))?,
                        })
                        .and_then(|op| {
                            if t.i32(4, 1) != 1 || t.i32(5, 1) != 1 {
                                Err(Refusal::Unsupported("dilated CONV_2D".into()))
                            } else {
                                Ok(op)
                            }
                        })
                    })?);
                }
                4 => ops.push(o(&|t| {
                    if t.i32(3, 1) != 1 {
                        return Err(Refusal::Unsupported("depth multiplier".into()));
                    }
                    if t.i32(5, 1) != 1 || t.i32(6, 1) != 1 {
                        return Err(Refusal::Unsupported("dilated DEPTHWISE_CONV_2D".into()));
                    }
                    Ok(Op::Depthwise {
                        x: i(0)?,
                        w: i(1)?,
                        b: i(2)?,
                        y,
                        pad: pad_of(t.u8(0, 0)),
                        sw: us(t.i32(1, 1)),
                        sh: us(t.i32(2, 1)),
                        act: act_of(t.u8(4, 0))?,
                    })
                })?),
                1 => ops.push(o(&|t| {
                    Ok(Op::AvgPool {
                        x: i(0)?,
                        y,
                        pad: pad_of(t.u8(0, 0)),
                        sw: us(t.i32(1, 1)),
                        sh: us(t.i32(2, 1)),
                        fw: us(t.i32(3, 1)),
                        fh: us(t.i32(4, 1)),
                        act: act_of(t.u8(5, 0))?,
                    })
                })?),
                0 => ops.push(Op::Add {
                    a: i(0)?,
                    b: i(1)?,
                    y,
                    act: act_of(opts.map_or(0, |t| t.u8(0, 0)))?,
                }),
                18 => ops.push(Op::Mul {
                    a: i(0)?,
                    b: i(1)?,
                    y,
                    act: act_of(opts.map_or(0, |t| t.u8(0, 0)))?,
                }),
                19 => ops.push(Op::Relu { x: i(0)?, y }),
                117 => ops.push(Op::HardSwish { x: i(0)?, y }),
                14 => ops.push(Op::Logistic { x: i(0)?, y }),
                23 => {
                    let (align, half) =
                        opts.map_or((false, false), |t| (t.bool(2, false), t.bool(3, false)));
                    // The size tensor is a constant, and the output shape already
                    // says the same thing — it is read only to refuse a disagreement.
                    let size = ints[i(1)?].clone().unwrap_or_default();
                    let out = shapes[y];
                    if size.len() == 2 && (us(size[0]) != out[1] || us(size[1]) != out[2]) {
                        return Err(Refusal::Malformed("resize size disagrees with its output"));
                    }
                    ops.push(Op::Resize {
                        x: i(0)?,
                        y,
                        align,
                        half,
                    });
                }
                32 => {
                    let name = code.str(1).unwrap_or("");
                    if name != "Convolution2DTransposeBias" {
                        return Err(Refusal::Unsupported(format!("custom {name}")));
                    }
                    // `TfLiteTransposeConvParams`: padding (0 unknown, 1 same,
                    // 2 valid), stride width, stride height — three `i32`s.
                    let raw = op.bytes_of(5).unwrap_or(&[]);
                    let word = |n: usize| {
                        raw.get(4 * n..4 * n + 4)
                            .map_or(1, |c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    };
                    ops.push(Op::TransposeBias {
                        x: i(0)?,
                        w: i(1)?,
                        b: i(2)?,
                        y,
                        pad: if word(0) == 2 { Pad::Valid } else { Pad::Same },
                        sw: us(word(1)),
                        sh: us(word(2)),
                    });
                }
                17 => ops.push(o(&|t| {
                    Ok(Op::MaxPool {
                        x: i(0)?,
                        y,
                        pad: pad_of(t.u8(0, 0)),
                        sw: us(t.i32(1, 1)),
                        sh: us(t.i32(2, 1)),
                        fw: us(t.i32(3, 1)),
                        fh: us(t.i32(4, 1)),
                        act: act_of(t.u8(5, 0))?,
                    })
                })?),
                54 => ops.push(Op::Prelu {
                    x: i(0)?,
                    alpha: i(1)?,
                    y,
                }),
                34 => {
                    let x = i(0)?;
                    let raw = ints[i(1)?]
                        .clone()
                        .ok_or_else(|| Refusal::Unsupported("PAD by a live tensor".into()))?;
                    let rank = ranks[x];
                    if raw.len() != 2 * rank || raw.iter().any(|p| *p < 0) {
                        return Err(Refusal::Malformed("pad amounts"));
                    }
                    let mut pads = [[0usize; 2]; 4];
                    for axis in 0..rank {
                        pads[4 - rank + axis] = [
                            usize::try_from(raw[2 * axis]).unwrap_or(0),
                            usize::try_from(raw[2 * axis + 1]).unwrap_or(0),
                        ];
                    }
                    ops.push(Op::Pad { x, y, pads });
                }
                22 => ops.push(Op::Reshape { x: i(0)?, y }),
                2 => {
                    let rank = ranks[y];
                    let raw = opts.map_or(0, |t| t.i32(0, 0));
                    let axis = if raw < 0 { rank as i32 + raw } else { raw };
                    let axis = usize::try_from(axis)
                        .ok()
                        .filter(|a| *a < rank)
                        .ok_or(Refusal::Malformed("concatenation axis"))?;
                    let xs = (0..ins.len()).map(i).collect::<Result<Vec<_>, _>>()?;
                    ops.push(Op::Concat {
                        xs,
                        y,
                        axis: axis + 4 - rank,
                        act: act_of(opts.map_or(0, |t| t.u8(1, 0)))?,
                    });
                }
                40 => {
                    let x = i(0)?;
                    let axes = ints[i(1)?].clone().unwrap_or_default();
                    let rank = ranks[x] as i32;
                    let mut axes: Vec<i32> = axes
                        .iter()
                        .map(|a| if *a < 0 { rank + a } else { *a })
                        .collect();
                    axes.sort_unstable();
                    if rank != 4 || axes != [1, 2] {
                        return Err(Refusal::Unsupported("MEAN over other axes".into()));
                    }
                    ops.push(Op::MeanSpatial { x, y });
                }
                9 => {
                    if opts.is_some_and(|t| t.u8(1, 0) != 0) {
                        return Err(Refusal::Unsupported(
                            "shuffled FULLY_CONNECTED weights".into(),
                        ));
                    }
                    let b = match ins.get(2) {
                        Some(v) if *v >= 0 => Some(slot(*v)?),
                        _ => None,
                    };
                    ops.push(Op::FullyConnected {
                        x: i(0)?,
                        w: i(1)?,
                        b,
                        y,
                        act: act_of(opts.map_or(0, |t| t.u8(0, 0)))?,
                    });
                }
                n => return Err(Refusal::Unsupported(format!("builtin {n}"))),
            }
        }

        let input = slot(*inputs.first().ok_or(Refusal::Malformed("no input"))?)?;
        let outputs = outputs
            .iter()
            .map(|o| slot(*o))
            .collect::<Result<Vec<_>, _>>()?;
        if outputs.is_empty() {
            return Err(Refusal::Malformed("no output"));
        }
        Ok(Self {
            shapes,
            consts,
            ops,
            input,
            outputs,
            threads: 1,
        })
    }

    /// The input's height, width and channels.
    pub fn input_shape(&self) -> [usize; 3] {
        let s = self.shapes[self.input];
        [s[1], s[2], s[3]]
    }

    /// The first output's height, width and channels.
    pub fn output_shape(&self) -> [usize; 3] {
        let s = self.shapes[self.outputs[0]];
        [s[1], s[2], s[3]]
    }

    /// Splits the two convolutions over up to `threads` threads. One (the
    /// default) never spawns: the camera's segmentation runs beside the
    /// effects, which are the frame's budget.
    pub fn set_threads(&mut self, threads: usize) {
        self.threads = threads.max(1);
    }

    /// How many outputs the graph has.
    pub fn outputs(&self) -> usize {
        self.outputs.len()
    }

    /// Runs the graph on an NHWC input and gives back the output tensor.
    ///
    /// # Panics
    ///
    /// When `input` is not exactly the size [`Net::input_shape`] says — a
    /// caller error, not a data error.
    pub fn run<'s>(&'s self, input: &[f32], scratch: &'s mut Scratch) -> &'s [f32] {
        self.run_all(input, scratch);
        self.output(scratch, 0)
    }

    /// Output `n` of the last [`Net::run_all`] on `scratch`, in the order the
    /// graph declares them.
    pub fn output<'s>(&'s self, scratch: &'s Scratch, n: usize) -> &'s [f32] {
        let t = self.outputs[n];
        self.consts[t].as_deref().unwrap_or(&scratch.values[t])
    }

    /// Runs the graph; every output is then read with [`Net::output`].
    ///
    /// # Panics
    ///
    /// As [`Net::run`].
    pub fn run_all(&self, input: &[f32], scratch: &mut Scratch) {
        let [h, w, c] = self.input_shape();
        assert_eq!(input.len(), h * w * c, "input size");
        scratch.values.resize_with(self.shapes.len(), Vec::new);
        let v = &mut scratch.values;
        v[self.input].clear();
        v[self.input].extend_from_slice(input);
        for op in &self.ops {
            self.step(op, v);
        }
    }

    fn get<'a>(&'a self, v: &'a [Vec<f32>], t: usize) -> &'a [f32] {
        self.consts[t].as_deref().unwrap_or(&v[t])
    }

    fn step(&self, op: &Op, v: &mut [Vec<f32>]) {
        let mut out = std::mem::take(&mut v[out_of(op)]);
        let size = {
            let s = self.shapes[out_of(op)];
            s[0] * s[1] * s[2] * s[3]
        };
        out.clear();
        out.resize(size, 0.0);
        match *op {
            Op::Conv {
                x,
                w,
                b,
                y,
                pad,
                sw,
                sh,
                act,
            } => {
                self.conv(
                    self.get(v, x),
                    self.get(v, w),
                    self.get(v, b),
                    &mut out,
                    [x, w, y],
                    pad,
                    [sw, sh],
                    act,
                );
            }
            Op::Depthwise {
                x,
                w,
                b,
                y,
                pad,
                sw,
                sh,
                act,
            } => {
                self.depthwise(
                    self.get(v, x),
                    self.get(v, w),
                    self.get(v, b),
                    &mut out,
                    [x, w, y],
                    pad,
                    [sw, sh],
                    act,
                );
            }
            Op::AvgPool {
                x,
                y,
                pad,
                sw,
                sh,
                fw,
                fh,
                act,
            } => {
                self.pool(
                    self.get(v, x),
                    &mut out,
                    [x, y],
                    pad,
                    [sw, sh],
                    [fw, fh],
                    act,
                );
            }
            Op::Add { a, b, y, act } => self.broadcast(
                self.get(v, a),
                self.get(v, b),
                &mut out,
                [a, b, y],
                act,
                |p, q| p + q,
            ),
            Op::Mul { a, b, y, act } => self.broadcast(
                self.get(v, a),
                self.get(v, b),
                &mut out,
                [a, b, y],
                act,
                |p, q| p * q,
            ),
            Op::Relu { x, .. } => {
                for (o, i) in out.iter_mut().zip(self.get(v, x)) {
                    *o = i.max(0.0);
                }
            }
            Op::HardSwish { x, .. } => {
                for (o, i) in out.iter_mut().zip(self.get(v, x)) {
                    *o = i * (i + 3.0).clamp(0.0, 6.0) / 6.0;
                }
            }
            Op::Logistic { x, .. } => {
                for (o, i) in out.iter_mut().zip(self.get(v, x)) {
                    *o = 1.0 / (1.0 + (-i).exp());
                }
            }
            Op::Resize { x, y, align, half } => {
                self.resize(self.get(v, x), &mut out, [x, y], align, half)
            }
            Op::TransposeBias {
                x,
                w,
                b,
                y,
                pad,
                sw,
                sh,
            } => {
                self.transpose(
                    self.get(v, x),
                    self.get(v, w),
                    self.get(v, b),
                    &mut out,
                    [x, w, y],
                    pad,
                    [sw, sh],
                );
            }
            Op::MaxPool {
                x,
                y,
                pad,
                sw,
                sh,
                fw,
                fh,
                act,
            } => self.max_pool(
                self.get(v, x),
                &mut out,
                [x, y],
                pad,
                [sw, sh],
                [fw, fh],
                act,
            ),
            Op::Prelu { x, alpha, y } => self.broadcast(
                self.get(v, x),
                self.get(v, alpha),
                &mut out,
                [x, alpha, y],
                Act::None,
                |p, a| if p >= 0.0 { p } else { p * a },
            ),
            Op::Pad { x, y, pads } => self.pad(self.get(v, x), &mut out, [x, y], pads),
            Op::Reshape { x, .. } => out.copy_from_slice(self.get(v, x)),
            Op::Concat {
                ref xs,
                y,
                axis,
                act,
            } => {
                let so = self.shapes[y];
                let outer: usize = so[..axis].iter().product();
                let mut k = 0;
                for n in 0..outer {
                    for &t in xs {
                        let inner: usize = self.shapes[t][axis..].iter().product();
                        let src = &self.get(v, t)[n * inner..][..inner];
                        for (d, s) in out[k..k + inner].iter_mut().zip(src) {
                            *d = activate(*s, act);
                        }
                        k += inner;
                    }
                }
            }
            Op::MeanSpatial { x, .. } => {
                let [_, h, w, c] = self.shapes[x];
                let src = self.get(v, x);
                for p in 0..h * w {
                    for (d, s) in out[..c].iter_mut().zip(&src[p * c..][..c]) {
                        *d += s;
                    }
                }
                let n = (h * w).max(1) as f32;
                for d in &mut out[..c] {
                    *d /= n;
                }
            }
            Op::FullyConnected { x, w, b, y, act } => {
                let [_, _, _, units] = self.shapes[y];
                let weights = self.get(v, w);
                let depth = weights.len() / units.max(1);
                let src = self.get(v, x);
                let bias = b.map(|b| self.get(v, b));
                let rows = src.len() / depth.max(1);
                for r in 0..rows {
                    let row = &src[r * depth..][..depth];
                    for u in 0..units {
                        let sum =
                            dot(row, &weights[u * depth..][..depth]) + bias.map_or(0.0, |b| b[u]);
                        out[r * units + u] = activate(sum, act);
                    }
                }
            }
        }
        v[out_of(op)] = out;
    }

    #[allow(clippy::too_many_arguments)]
    fn conv(
        &self,
        x: &[f32],
        w: &[f32],
        b: &[f32],
        out: &mut [f32],
        t: [usize; 3],
        pad: Pad,
        s: [usize; 2],
        act: Act,
    ) {
        let [_, ih, iw, ic] = self.shapes[t[0]];
        let [oc, kh, kw, _] = self.shapes[t[1]];
        let [_, oh, ow, _] = self.shapes[t[2]];
        let (pt, pl) = padding(pad, [ih, iw], [oh, ow], [kh, kw], s);
        // `w` is PACKED (`[kh][kw][ic][oc]`, see `Net::read`): each input
        // value is spread over every output channel at once.
        let work = oh * ow * oc * kh * kw * ic;
        by_rows(out, ow * oc, self.threads, work, |oy, row| {
            for ox in 0..ow {
                let acc = &mut row[ox * oc..][..oc];
                acc.copy_from_slice(&b[..oc]);
                for ky in 0..kh {
                    let iy = (oy * s[1] + ky) as isize - pt as isize;
                    if iy < 0 || iy >= ih as isize {
                        continue;
                    }
                    for kx in 0..kw {
                        let ix = (ox * s[0] + kx) as isize - pl as isize;
                        if ix < 0 || ix >= iw as isize {
                            continue;
                        }
                        let px = &x[(iy as usize * iw + ix as usize) * ic..][..ic];
                        let wk = &w[(ky * kw + kx) * ic * oc..][..ic * oc];
                        for (p, row) in px.iter().zip(wk.chunks_exact(oc)) {
                            for (a, q) in acc.iter_mut().zip(row) {
                                *a += p * q;
                            }
                        }
                    }
                }
                for a in acc.iter_mut() {
                    *a = activate(*a, act);
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn depthwise(
        &self,
        x: &[f32],
        w: &[f32],
        b: &[f32],
        out: &mut [f32],
        t: [usize; 3],
        pad: Pad,
        s: [usize; 2],
        act: Act,
    ) {
        let [_, ih, iw, c] = self.shapes[t[0]];
        let [_, kh, kw, _] = self.shapes[t[1]];
        let [_, oh, ow, _] = self.shapes[t[2]];
        let (pt, pl) = padding(pad, [ih, iw], [oh, ow], [kh, kw], s);
        let work = oh * ow * c * kh * kw;
        by_rows(out, ow * c, self.threads, work, |oy, row| {
            for ox in 0..ow {
                let dst = &mut row[ox * c..][..c];
                dst.copy_from_slice(&b[..c]);
                for ky in 0..kh {
                    let iy = (oy * s[1] + ky) as isize - pt as isize;
                    if iy < 0 || iy >= ih as isize {
                        continue;
                    }
                    for kx in 0..kw {
                        let ix = (ox * s[0] + kx) as isize - pl as isize;
                        if ix < 0 || ix >= iw as isize {
                            continue;
                        }
                        let px = &x[(iy as usize * iw + ix as usize) * c..][..c];
                        let wk = &w[(ky * kw + kx) * c..][..c];
                        for ((d, p), q) in dst.iter_mut().zip(px).zip(wk) {
                            *d += p * q;
                        }
                    }
                }
                for d in dst.iter_mut() {
                    *d = activate(*d, act);
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn pool(
        &self,
        x: &[f32],
        out: &mut [f32],
        t: [usize; 2],
        pad: Pad,
        s: [usize; 2],
        f: [usize; 2],
        act: Act,
    ) {
        let [_, ih, iw, c] = self.shapes[t[0]];
        let [_, oh, ow, _] = self.shapes[t[1]];
        let (pt, pl) = padding(pad, [ih, iw], [oh, ow], [f[1], f[0]], s);
        for oy in 0..oh {
            for ox in 0..ow {
                let y0 = (oy * s[1]) as isize - pt as isize;
                let x0 = (ox * s[0]) as isize - pl as isize;
                let dst = &mut out[(oy * ow + ox) * c..][..c];
                let mut n = 0.0f32;
                for iy in y0.max(0)..(y0 + f[1] as isize).min(ih as isize) {
                    for ix in x0.max(0)..(x0 + f[0] as isize).min(iw as isize) {
                        let px = &x[(iy as usize * iw + ix as usize) * c..][..c];
                        for (d, p) in dst.iter_mut().zip(px) {
                            *d += p;
                        }
                        n += 1.0;
                    }
                }
                for d in dst.iter_mut() {
                    *d = activate(*d / n.max(1.0), act);
                }
            }
        }
    }

    fn broadcast(
        &self,
        a: &[f32],
        b: &[f32],
        out: &mut [f32],
        t: [usize; 3],
        act: Act,
        f: impl Fn(f32, f32) -> f32,
    ) {
        let sa = self.shapes[t[0]];
        let sb = self.shapes[t[1]];
        let so = self.shapes[t[2]];
        if sa == so && sb == so {
            for ((o, p), q) in out.iter_mut().zip(a).zip(b) {
                *o = activate(f(*p, *q), act);
            }
            return;
        }
        // One value per CHANNEL on the right (PReLU's slopes, a bias): the
        // general index arithmetic below cost the palm detector 7.6 ms of 52.
        if sa == so && sb[..3] == [1, 1, 1] && sb[3] == so[3] {
            for (o, p) in out.chunks_exact_mut(so[3]).zip(a.chunks_exact(so[3])) {
                for ((o, p), q) in o.iter_mut().zip(p).zip(b) {
                    *o = activate(f(*p, *q), act);
                }
            }
            return;
        }
        let index = |s: [usize; 4], n: usize, y: usize, x: usize, c: usize| {
            let n = if s[0] == 1 { 0 } else { n };
            let y = if s[1] == 1 { 0 } else { y };
            let x = if s[2] == 1 { 0 } else { x };
            let c = if s[3] == 1 { 0 } else { c };
            ((n * s[1] + y) * s[2] + x) * s[3] + c
        };
        let mut k = 0;
        for n in 0..so[0] {
            for y in 0..so[1] {
                for x in 0..so[2] {
                    for c in 0..so[3] {
                        out[k] =
                            activate(f(a[index(sa, n, y, x, c)], b[index(sb, n, y, x, c)]), act);
                        k += 1;
                    }
                }
            }
        }
    }

    fn resize(&self, x: &[f32], out: &mut [f32], t: [usize; 2], align: bool, half: bool) {
        let [_, ih, iw, c] = self.shapes[t[0]];
        let [_, oh, ow, _] = self.shapes[t[1]];
        let scale = |i: usize, o: usize| {
            if align && o > 1 {
                (i - 1) as f32 / (o - 1) as f32
            } else {
                i as f32 / o as f32
            }
        };
        let (hs, ws) = (scale(ih, oh), scale(iw, ow));
        let at = |v: usize, s: f32, size: usize| {
            let p = if half {
                (v as f32 + 0.5) * s - 0.5
            } else {
                v as f32 * s
            };
            let lo = (p.floor() as isize).max(0) as usize;
            let hi = (p.ceil() as isize).min(size as isize - 1).max(0) as usize;
            (p, lo, hi)
        };
        for oy in 0..oh {
            let (py, y0, y1) = at(oy, hs, ih);
            let fy = py - y0 as f32;
            for ox in 0..ow {
                let (px, x0, x1) = at(ox, ws, iw);
                let fx = px - x0 as f32;
                let dst = &mut out[(oy * ow + ox) * c..][..c];
                let p00 = &x[(y0 * iw + x0) * c..][..c];
                let p01 = &x[(y0 * iw + x1) * c..][..c];
                let p10 = &x[(y1 * iw + x0) * c..][..c];
                let p11 = &x[(y1 * iw + x1) * c..][..c];
                for k in 0..c {
                    dst[k] = p00[k] * (1.0 - fy) * (1.0 - fx)
                        + p10[k] * fy * (1.0 - fx)
                        + p01[k] * (1.0 - fy) * fx
                        + p11[k] * fy * fx;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn transpose(
        &self,
        x: &[f32],
        w: &[f32],
        b: &[f32],
        out: &mut [f32],
        t: [usize; 3],
        pad: Pad,
        s: [usize; 2],
    ) {
        let [_, ih, iw, ic] = self.shapes[t[0]];
        let [oc, kh, kw, _] = self.shapes[t[1]];
        let [_, oh, ow, _] = self.shapes[t[2]];
        // The transposed convolution's padding is the forward one read backwards:
        // the output is what would have been the input.
        let (pt, pl) = padding(pad, [oh, ow], [ih, iw], [kh, kw], s);
        for o in 0..oh * ow {
            out[o * oc..][..oc].copy_from_slice(&b[..oc]);
        }
        for iy in 0..ih {
            for ix in 0..iw {
                let px = &x[(iy * iw + ix) * ic..][..ic];
                for ky in 0..kh {
                    let oy = (iy * s[1] + ky) as isize - pt as isize;
                    if oy < 0 || oy >= oh as isize {
                        continue;
                    }
                    for kx in 0..kw {
                        let ox = (ix * s[0] + kx) as isize - pl as isize;
                        if ox < 0 || ox >= ow as isize {
                            continue;
                        }
                        let dst = &mut out[(oy as usize * ow + ox as usize) * oc..][..oc];
                        for (o, d) in dst.iter_mut().enumerate() {
                            *d += dot(px, &w[((o * kh + ky) * kw + kx) * ic..][..ic]);
                        }
                    }
                }
            }
        }
    }
}

impl Net {
    #[allow(clippy::too_many_arguments)]
    fn max_pool(
        &self,
        x: &[f32],
        out: &mut [f32],
        t: [usize; 2],
        pad: Pad,
        s: [usize; 2],
        f: [usize; 2],
        act: Act,
    ) {
        let [_, ih, iw, c] = self.shapes[t[0]];
        let [_, oh, ow, _] = self.shapes[t[1]];
        let (pt, pl) = padding(pad, [ih, iw], [oh, ow], [f[1], f[0]], s);
        for oy in 0..oh {
            for ox in 0..ow {
                let y0 = (oy * s[1]) as isize - pt as isize;
                let x0 = (ox * s[0]) as isize - pl as isize;
                let dst = &mut out[(oy * ow + ox) * c..][..c];
                dst.fill(f32::NEG_INFINITY);
                for iy in y0.max(0)..(y0 + f[1] as isize).min(ih as isize) {
                    for ix in x0.max(0)..(x0 + f[0] as isize).min(iw as isize) {
                        let px = &x[(iy as usize * iw + ix as usize) * c..][..c];
                        for (d, p) in dst.iter_mut().zip(px) {
                            *d = d.max(*p);
                        }
                    }
                }
                for d in dst.iter_mut() {
                    *d = activate(*d, act);
                }
            }
        }
    }

    /// Zero padding on every axis; `out` arrives zeroed.
    fn pad(&self, x: &[f32], out: &mut [f32], t: [usize; 2], pads: [[usize; 2]; 4]) {
        let si = self.shapes[t[0]];
        let so = self.shapes[t[1]];
        let c = si[3];
        for n in 0..si[0] {
            for yy in 0..si[1] {
                for xx in 0..si[2] {
                    let src = &x[((n * si[1] + yy) * si[2] + xx) * c..][..c];
                    let at =
                        (((n + pads[0][0]) * so[1] + yy + pads[1][0]) * so[2] + xx + pads[2][0])
                            * so[3]
                            + pads[3][0];
                    out[at..at + c].copy_from_slice(src);
                }
            }
        }
    }
}

fn out_of(op: &Op) -> usize {
    match *op {
        Op::Conv { y, .. }
        | Op::Depthwise { y, .. }
        | Op::AvgPool { y, .. }
        | Op::Add { y, .. }
        | Op::Mul { y, .. }
        | Op::Relu { y, .. }
        | Op::HardSwish { y, .. }
        | Op::Logistic { y, .. }
        | Op::Resize { y, .. }
        | Op::TransposeBias { y, .. }
        | Op::MaxPool { y, .. }
        | Op::Prelu { y, .. }
        | Op::Pad { y, .. }
        | Op::Reshape { y, .. }
        | Op::Concat { y, .. }
        | Op::MeanSpatial { y, .. }
        | Op::FullyConnected { y, .. } => y,
    }
}

/// TFLite's `ComputePaddingHeightWidth`: the SMALLER half goes before.
fn padding(
    pad: Pad,
    input: [usize; 2],
    output: [usize; 2],
    k: [usize; 2],
    s: [usize; 2],
) -> (usize, usize) {
    if pad == Pad::Valid {
        return (0, 0);
    }
    let one = |i: usize, o: usize, k: usize, s: usize| {
        let total = ((o.saturating_sub(1)) * s + k).saturating_sub(i);
        total / 2
    };
    (
        one(input[0], output[0], k[0], s[1]),
        one(input[1], output[1], k[1], s[0]),
    )
}

fn activate(v: f32, act: Act) -> f32 {
    match act {
        Act::None => v,
        Act::Relu => v.max(0.0),
        Act::Relu6 => v.clamp(0.0, 6.0),
    }
}

/// Below this many multiplications an operator is not worth a thread: spawning
/// one costs tens of microseconds.
const WORTH_A_THREAD: usize = 200_000;

/// Calls `f(row, slice)` for every output row, split into bands over up to
/// `threads` scoped threads when `work` is worth it.
fn by_rows(
    out: &mut [f32],
    row_len: usize,
    threads: usize,
    work: usize,
    f: impl Fn(usize, &mut [f32]) + Sync,
) {
    let rows = out.len() / row_len.max(1);
    if threads <= 1 || rows < 2 || work < WORTH_A_THREAD {
        for (r, row) in out.chunks_exact_mut(row_len).enumerate() {
            f(r, row);
        }
        return;
    }
    let per = rows.div_ceil(threads.min(rows));
    std::thread::scope(|s| {
        for (t, band) in out.chunks_mut(per * row_len).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (k, row) in band.chunks_exact_mut(row_len).enumerate() {
                    f(t * per + k, row);
                }
            });
        }
    });
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    // Four lanes, which is what lets the compiler vectorise without `unsafe`.
    let mut acc = [0.0f32; 4];
    let chunks = a.len() / 4 * 4;
    for i in (0..chunks).step_by(4) {
        acc[0] += a[i] * b[i];
        acc[1] += a[i + 1] * b[i + 1];
        acc[2] += a[i + 2] * b[i + 2];
        acc[3] += a[i + 3] * b[i + 3];
    }
    let mut sum = acc[0] + acc[1] + acc[2] + acc[3];
    for i in chunks..a.len() {
        sum += a[i] * b[i];
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats_read_back() {
        assert_eq!(half(0x3c00), 1.0);
        assert_eq!(half(0xc000), -2.0);
        assert_eq!(half(0x0000), 0.0);
        assert_eq!(half(0x7bff), 65504.0);
        assert!((half(0x0001) - 5.960_464_5e-8).abs() < 1e-12, "subnormal");
        assert!(half(0x7c00).is_infinite());
    }

    #[test]
    fn same_padding_puts_the_smaller_half_first() {
        // 256 → 128 with a 3×3 at stride 2: total 1, so 0 before and 1 after.
        assert_eq!(
            padding(Pad::Same, [256, 256], [128, 128], [3, 3], [2, 2]),
            (0, 0)
        );
        // 32 → 32 with a 5×5 at stride 1: total 4, two before.
        assert_eq!(
            padding(Pad::Same, [32, 32], [32, 32], [5, 5], [1, 1]),
            (2, 2)
        );
    }

    #[test]
    fn garbage_is_a_refusal() {
        assert!(Net::read(&[1, 2, 3]).is_err());
    }
}
