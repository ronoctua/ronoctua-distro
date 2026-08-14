// Uma imagem numa area da tela — ver `image.rs`.
//
// Mesma economia do `quad.wgsl`: sem buffer de vertices, os quatro cantos
// saem do `vertex_index`. A diferenca e que aqui cada canto tambem carrega a
// coordenada de textura, e ha um bind group com a textura e o amostrador.

struct Instancia {
    // Canto superior esquerdo, em NDC.
    @location(0) pos: vec2<f32>,
    // Tamanho em NDC. O Y vem NEGATIVO: em NDC ele cresce para cima, e em
    // pixel para baixo.
    @location(1) tam: vec2<f32>,
};

struct Saida {
    @builtin(position) posicao: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var textura: texture_2d<f32>;
@group(0) @binding(1) var amostrador: sampler;

@vertex
fn vs(@builtin(vertex_index) vi: u32, inst: Instancia) -> Saida {
    let direita = f32(vi >> 1u);
    let baixo = f32(vi & 1u);

    var s: Saida;
    s.posicao = vec4<f32>(
        inst.pos.x + inst.tam.x * direita,
        inst.pos.y + inst.tam.y * baixo,
        0.0,
        1.0,
    );
    // A UV acompanha o canto: (0,0) no superior esquerdo da imagem. Ela NAO
    // inverte junto com o Y do NDC — a imagem ja vem com a primeira linha em
    // cima, como todo formato de arquivo entrega.
    s.uv = vec2<f32>(direita, baixo);
    return s;
}

@fragment
fn fs(entrada: Saida) -> @location(0) vec4<f32> {
    return textureSample(textura, amostrador, entrada.uv);
}
