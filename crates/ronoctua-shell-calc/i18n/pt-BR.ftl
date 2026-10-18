# The texts `ronoctua-shell-calc` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# THE CALCULATOR'S ENGINE (`ronoctua-calc`): why an expression has no value,
# the converter's categories, the shapes of a quick answer, and every unit's
# name — chosen by the number (`$count`, a number), as `assets/units.nix` declares.
calc-why-empty = Não há nada para calcular.
calc-why-unexpected = Algo aqui está fora do lugar.
calc-why-unbalanced = Um parêntese foi aberto e não fechado, ou fechado sem ter sido aberto.
calc-why-unknown-word = Uma palavra que não é unidade, moeda, função nem fuso horário.
calc-why-incompatible = São grandezas de tipos diferentes e não se combinam.
calc-why-divide-by-zero = Uma divisão por zero.
calc-why-too-large = O resultado é grande demais para caber.
calc-why-domain = A função não tem valor aí.
calc-why-no-rates = Converter entre moedas precisa das cotações, e ainda não há nenhuma.
calc-why-unknown-currency = As cotações não trazem esta moeda.
calc-why-unknown-zone = Nenhum fuso horário tem esse nome.
calc-why-bad-date = Essa data não existe.
calc-why-bad-roman = Isso não é um número romano (de 1 a 3999, na forma usual).
calc-why-bad-digit = Um algarismo que a base do número não tem.
calc-rates-why-wrong-pass = A permissão dada é para outro site, não o do Banco Central Europeu.
calc-rates-why-failed = O Banco Central Europeu respondeu com uma falha.
calc-rates-why-garbled = O que voltou não é o arquivo de cotações do Banco Central Europeu.
calc-category-length = Comprimento
calc-category-mass = Massa
calc-category-volume = Volume
calc-category-area = Área
calc-category-temperature = Temperatura
calc-category-time = Tempo
calc-category-speed = Velocidade
calc-category-data = Dados
calc-category-energy = Energia
calc-category-power = Potência
calc-category-pressure = Pressão
calc-category-angle = Ângulo
calc-category-frequency = Frequência
calc-category-fuel-economy = Consumo de combustível
calc-category-cooking = Culinária
unit-metre = { $count ->
        [one] metro
       *[other] metros
    }
unit-kilometre = { $count ->
        [one] quilômetro
       *[other] quilômetros
    }
unit-centimetre = { $count ->
        [one] centímetro
       *[other] centímetros
    }
unit-millimetre = { $count ->
        [one] milímetro
       *[other] milímetros
    }
unit-micrometre = { $count ->
        [one] micrômetro
       *[other] micrômetros
    }
unit-nanometre = { $count ->
        [one] nanômetro
       *[other] nanômetros
    }
unit-mile = { $count ->
        [one] milha
       *[other] milhas
    }
unit-yard = { $count ->
        [one] jarda
       *[other] jardas
    }
unit-foot = { $count ->
        [one] pé
       *[other] pés
    }
unit-inch = { $count ->
        [one] polegada
       *[other] polegadas
    }
unit-nautical-mile = { $count ->
        [one] milha náutica
       *[other] milhas náuticas
    }
unit-astronomical-unit = { $count ->
        [one] unidade astronômica
       *[other] unidades astronômicas
    }
unit-light-year = { $count ->
        [one] ano-luz
       *[other] anos-luz
    }
unit-kilogram = { $count ->
        [one] quilograma
       *[other] quilogramas
    }
unit-gram = { $count ->
        [one] grama
       *[other] gramas
    }
unit-milligram = { $count ->
        [one] miligrama
       *[other] miligramas
    }
unit-microgram = { $count ->
        [one] micrograma
       *[other] microgramas
    }
unit-tonne = { $count ->
        [one] tonelada
       *[other] toneladas
    }
unit-pound = { $count ->
        [one] libra
       *[other] libras
    }
unit-ounce = { $count ->
        [one] onça
       *[other] onças
    }
unit-stone = { $count ->
        [one] stone
       *[other] stones
    }
unit-carat = { $count ->
        [one] quilate
       *[other] quilates
    }
unit-short-ton = { $count ->
        [one] tonelada curta
       *[other] toneladas curtas
    }
unit-long-ton = { $count ->
        [one] tonelada longa
       *[other] toneladas longas
    }
unit-cubic-metre = { $count ->
        [one] metro cúbico
       *[other] metros cúbicos
    }
unit-litre = { $count ->
        [one] litro
       *[other] litros
    }
unit-decilitre = { $count ->
        [one] decilitro
       *[other] decilitros
    }
unit-centilitre = { $count ->
        [one] centilitro
       *[other] centilitros
    }
unit-millilitre = { $count ->
        [one] mililitro
       *[other] mililitros
    }
unit-cubic-centimetre = { $count ->
        [one] centímetro cúbico
       *[other] centímetros cúbicos
    }
unit-gallon = { $count ->
        [one] galão
       *[other] galões
    }
unit-imperial-gallon = { $count ->
        [one] galão imperial
       *[other] galões imperiais
    }
unit-quart = { $count ->
        [one] quarto de galão
       *[other] quartos de galão
    }
unit-pint = { $count ->
        [one] pinta
       *[other] pintas
    }
unit-fluid-ounce = { $count ->
        [one] onça líquida
       *[other] onças líquidas
    }
unit-cubic-foot = { $count ->
        [one] pé cúbico
       *[other] pés cúbicos
    }
unit-cubic-inch = { $count ->
        [one] polegada cúbica
       *[other] polegadas cúbicas
    }
unit-square-metre = { $count ->
        [one] metro quadrado
       *[other] metros quadrados
    }
unit-square-kilometre = { $count ->
        [one] quilômetro quadrado
       *[other] quilômetros quadrados
    }
unit-square-centimetre = { $count ->
        [one] centímetro quadrado
       *[other] centímetros quadrados
    }
unit-square-millimetre = { $count ->
        [one] milímetro quadrado
       *[other] milímetros quadrados
    }
unit-hectare = { $count ->
        [one] hectare
       *[other] hectares
    }
unit-acre = { $count ->
        [one] acre
       *[other] acres
    }
unit-square-foot = { $count ->
        [one] pé quadrado
       *[other] pés quadrados
    }
unit-square-inch = { $count ->
        [one] polegada quadrada
       *[other] polegadas quadradas
    }
unit-square-yard = { $count ->
        [one] jarda quadrada
       *[other] jardas quadradas
    }
unit-square-mile = { $count ->
        [one] milha quadrada
       *[other] milhas quadradas
    }
unit-kelvin = { $count ->
        [one] kelvin
       *[other] kelvins
    }
unit-celsius = { $count ->
        [one] grau Celsius
       *[other] graus Celsius
    }
unit-fahrenheit = { $count ->
        [one] grau Fahrenheit
       *[other] graus Fahrenheit
    }
unit-rankine = { $count ->
        [one] grau Rankine
       *[other] graus Rankine
    }
unit-second = { $count ->
        [one] segundo
       *[other] segundos
    }
unit-millisecond = { $count ->
        [one] milissegundo
       *[other] milissegundos
    }
unit-microsecond = { $count ->
        [one] microssegundo
       *[other] microssegundos
    }
unit-nanosecond = { $count ->
        [one] nanossegundo
       *[other] nanossegundos
    }
unit-minute = { $count ->
        [one] minuto
       *[other] minutos
    }
unit-hour = { $count ->
        [one] hora
       *[other] horas
    }
unit-day = { $count ->
        [one] dia
       *[other] dias
    }
unit-week = { $count ->
        [one] semana
       *[other] semanas
    }
unit-month = { $count ->
        [one] mês
       *[other] meses
    }
unit-year = { $count ->
        [one] ano
       *[other] anos
    }
unit-metre-per-second = { $count ->
        [one] metro por segundo
       *[other] metros por segundo
    }
unit-kilometre-per-hour = { $count ->
        [one] quilômetro por hora
       *[other] quilômetros por hora
    }
unit-mile-per-hour = { $count ->
        [one] milha por hora
       *[other] milhas por hora
    }
unit-knot = { $count ->
        [one] nó
       *[other] nós
    }
unit-foot-per-second = { $count ->
        [one] pé por segundo
       *[other] pés por segundo
    }
unit-bit = { $count ->
        [one] bit
       *[other] bits
    }
unit-byte = { $count ->
        [one] byte
       *[other] bytes
    }
unit-kilobit = { $count ->
        [one] quilobit
       *[other] quilobits
    }
unit-megabit = { $count ->
        [one] megabit
       *[other] megabits
    }
unit-gigabit = { $count ->
        [one] gigabit
       *[other] gigabits
    }
unit-kibibit = { $count ->
        [one] kibibit
       *[other] kibibits
    }
unit-mebibit = { $count ->
        [one] mebibit
       *[other] mebibits
    }
unit-kilobyte = { $count ->
        [one] quilobyte
       *[other] quilobytes
    }
unit-megabyte = { $count ->
        [one] megabyte
       *[other] megabytes
    }
unit-gigabyte = { $count ->
        [one] gigabyte
       *[other] gigabytes
    }
unit-terabyte = { $count ->
        [one] terabyte
       *[other] terabytes
    }
unit-petabyte = { $count ->
        [one] petabyte
       *[other] petabytes
    }
unit-kibibyte = { $count ->
        [one] kibibyte
       *[other] kibibytes
    }
unit-mebibyte = { $count ->
        [one] mebibyte
       *[other] mebibytes
    }
unit-gibibyte = { $count ->
        [one] gibibyte
       *[other] gibibytes
    }
unit-tebibyte = { $count ->
        [one] tebibyte
       *[other] tebibytes
    }
unit-pebibyte = { $count ->
        [one] pebibyte
       *[other] pebibytes
    }
unit-joule = { $count ->
        [one] joule
       *[other] joules
    }
unit-kilojoule = { $count ->
        [one] quilojoule
       *[other] quilojoules
    }
unit-megajoule = { $count ->
        [one] megajoule
       *[other] megajoules
    }
unit-calorie = { $count ->
        [one] caloria
       *[other] calorias
    }
unit-kilocalorie = { $count ->
        [one] quilocaloria
       *[other] quilocalorias
    }
unit-watt-hour = { $count ->
        [one] watt-hora
       *[other] watts-hora
    }
unit-kilowatt-hour = { $count ->
        [one] quilowatt-hora
       *[other] quilowatts-hora
    }
unit-electronvolt = { $count ->
        [one] elétron-volt
       *[other] elétrons-volt
    }
unit-british-thermal-unit = { $count ->
        [one] unidade térmica britânica
       *[other] unidades térmicas britânicas
    }
unit-watt = { $count ->
        [one] watt
       *[other] watts
    }
unit-kilowatt = { $count ->
        [one] quilowatt
       *[other] quilowatts
    }
unit-megawatt = { $count ->
        [one] megawatt
       *[other] megawatts
    }
unit-horsepower = horsepower
unit-metric-horsepower = { $count ->
        [one] cavalo-vapor
       *[other] cavalos-vapor
    }
unit-pascal = { $count ->
        [one] pascal
       *[other] pascals
    }
unit-hectopascal = { $count ->
        [one] hectopascal
       *[other] hectopascals
    }
unit-kilopascal = { $count ->
        [one] quilopascal
       *[other] quilopascals
    }
unit-megapascal = { $count ->
        [one] megapascal
       *[other] megapascals
    }
unit-bar = { $count ->
        [one] bar
       *[other] bares
    }
unit-millibar = { $count ->
        [one] milibar
       *[other] milibares
    }
unit-atmosphere = { $count ->
        [one] atmosfera
       *[other] atmosferas
    }
unit-pound-per-square-inch = { $count ->
        [one] libra por polegada quadrada
       *[other] libras por polegada quadrada
    }
unit-millimetre-of-mercury = { $count ->
        [one] milímetro de mercúrio
       *[other] milímetros de mercúrio
    }
unit-torr = { $count ->
        [one] torr
       *[other] torrs
    }
unit-radian = { $count ->
        [one] radiano
       *[other] radianos
    }
unit-degree = { $count ->
        [one] grau
       *[other] graus
    }
unit-gradian = { $count ->
        [one] grado
       *[other] grados
    }
unit-arcminute = { $count ->
        [one] minuto de arco
       *[other] minutos de arco
    }
unit-arcsecond = { $count ->
        [one] segundo de arco
       *[other] segundos de arco
    }
unit-turn = { $count ->
        [one] volta
       *[other] voltas
    }
unit-hertz = hertz
unit-kilohertz = quilohertz
unit-megahertz = megahertz
unit-gigahertz = gigahertz
unit-revolution-per-minute = { $count ->
        [one] rotação por minuto
       *[other] rotações por minuto
    }
unit-litre-per-100-km = { $count ->
        [one] litro por 100 quilômetros
       *[other] litros por 100 quilômetros
    }
unit-kilometre-per-litre = { $count ->
        [one] quilômetro por litro
       *[other] quilômetros por litro
    }
unit-mile-per-gallon = { $count ->
        [one] milha por galão
       *[other] milhas por galão
    }
unit-mile-per-imperial-gallon = { $count ->
        [one] milha por galão imperial
       *[other] milhas por galão imperial
    }
unit-cup = { $count ->
        [one] xícara
       *[other] xícaras
    }
unit-customary-cup = { $count ->
        [one] xícara americana
       *[other] xícaras americanas
    }
unit-metric-cup = { $count ->
        [one] xícara métrica
       *[other] xícaras métricas
    }
unit-tablespoon = { $count ->
        [one] colher de sopa
       *[other] colheres de sopa
    }
unit-customary-tablespoon = { $count ->
        [one] colher de sopa americana
       *[other] colheres de sopa americanas
    }
unit-teaspoon = { $count ->
        [one] colher de chá
       *[other] colheres de chá
    }
unit-customary-teaspoon = { $count ->
        [one] colher de chá americana
       *[other] colheres de chá americanas
    }

## file info editing — everything the info panel can change

# THE CALCULATOR (`LEADER =`, `crate::calc`): the leader's group and its two
# doors, the window's name and the modeline's name for it.
group-calc = calculadora
cmd-calc-open = A calculadora, no rodapé
cmd-calc-window = A calculadora, numa janela própria
calc-title = Calculadora
calc-mode-basic = Básica
calc-mode-scientific = Científica
calc-mode-convert = Conversão
calc-sum = Conta
calc-amount = Quantia
calc-empty = digite uma conta, ou use as teclas
calc-equals-exact = = { $value }
calc-equals-approx = ≈ { $value }
calc-history-title = Histórico
calc-history-empty = Nenhum resultado guardado ainda: { $keys } guarda um.
calc-from = De
calc-to = Para
calc-find = Procurar unidade — { $side }:
calc-find-none = Nenhuma unidade desta categoria tem esse nome.
calc-category-currency = Moeda
calc-rates-asking = Perguntando a { $host } as cotações do dia…
calc-rates-of = Cotações de { $date }, do Banco Central Europeu.
calc-unkept-unparsed = { $file } não pode ser lido, então este resultado não foi guardado; o arquivo ficou como estava.
calc-unkept = Este resultado não pôde ser guardado em { $file }: { $why }
calc-float-unreached = O compositor não pôde ser chamado para mover esta janela.
# The header's words, and what each one does under the pointer.
calc-equals = guardar
about-calc-equals = Guarda o resultado no histórico e continua a partir dele; num resultado escolhido no histórico, traz a conta dele de volta.
calc-mode = modo
about-calc-mode = Vai para o próximo modo, em volta dos três: básica, científica, conversão.
calc-clear = limpar
about-calc-clear = Esvazia a conta.
calc-copy = copiar
about-calc-copy = Copia o resultado — o escolhido no histórico, ou o que está na linha.
calc-history = histórico
about-calc-history = Escolhe um resultado do histórico, um mais antigo a cada vez.
calc-take = escolher
about-calc-take = Escolhe a unidade sob o cursor para o lado que está sendo escolhido.
calc-ring-next = próxima categoria
about-calc-ring-next = Vai para a próxima categoria de unidades.
calc-ring-previous = categoria anterior
about-calc-ring-previous = Vai para a categoria de unidades anterior.
about-calc-from = Escolhe a unidade da quantia: o que for digitado em seguida a procura.
about-calc-to = Escolhe a unidade para a qual a quantia é convertida: o que for digitado em seguida a procura.
calc-swap = trocar
about-calc-swap = Troca as duas unidades: o que era convertido de passa a ser convertido para.
calc-float = compacta ou normal
about-calc-float = Deixa a janela compacta, flutuando sobre as outras — ou a devolve ao layout entre elas.
calc-keys-narrow = As teclas das funções precisam de mais espaço que este. Digite-as pelo nome: sen, cos, √, ln…
# The keypad's legends: what is not a digit or the decimal mark.
calc-key-add = +
calc-key-subtract = −
calc-key-multiply = ×
calc-key-divide = ÷
calc-key-open = (
calc-key-close = )
calc-key-percent = %
calc-key-power = xʸ
calc-key-square = x²
calc-key-reciprocal = x⁻¹
calc-key-factorial = n!
calc-key-degree = °
calc-key-sin = sen
calc-key-cos = cos
calc-key-tan = tg
calc-key-asin = arcsen
calc-key-acos = arccos
calc-key-atan = arctg
calc-key-ln = ln
calc-key-log = log
calc-key-sqrt = √
calc-key-cbrt = ∛
calc-key-exp = exp
calc-key-pi = π
calc-key-e = e
calc-key-clear = C
calc-key-equals = =
