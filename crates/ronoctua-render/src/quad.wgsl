// Retangulo de cor chapada — ver `quad.rs`.
//
// Sem buffer de vertices e sem bind group: os quatro cantos saem do
// `vertex_index`, e a conversao para NDC ja veio pronta da CPU. O pipeline
// nao tem estado nenhum alem das instancias.

struct Instancia {
    // Canto superior esquerdo, em NDC.
    @location(0) pos: vec2<f32>,
    // Tamanho em NDC. O Y vem NEGATIVO: em NDC ele cresce para cima, e em
    // pixel para baixo.
    @location(1) tam: vec2<f32>,
    @location(2) cor: vec4<f32>,
};

struct Saida {
    @builtin(position) posicao: vec4<f32>,
    @location(0) cor: vec4<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, inst: Instancia) -> Saida {
    // `TriangleStrip` com quatro vertices, nesta ordem:
    //   0 = superior esquerdo   1 = inferior esquerdo
    //   2 = superior direito    3 = inferior direito
    //
    // O bit 1 diz se e da direita, o bit 0 se e de baixo. Uma tabela seria
    // mais legivel e ocuparia registrador a toa.
    let direita = f32(vi >> 1u);
    let baixo = f32(vi & 1u);

    var s: Saida;
    s.posicao = vec4<f32>(
        inst.pos.x + inst.tam.x * direita,
        inst.pos.y + inst.tam.y * baixo,
        0.0,
        1.0,
    );
    s.cor = inst.cor;
    return s;
}

@fragment
fn fs(entrada: Saida) -> @location(0) vec4<f32> {
    // Cor chapada. Sem gradiente, sem borda, sem raio — invariante 15.
    return entrada.cor;
}
