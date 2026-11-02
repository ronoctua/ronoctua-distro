# The security watch's own texts: its name and the line under it, the hint of
# each of its marks, its notice, and its page.
brain-task-security-watch = Vigiar a central de segurança
brain-task-security-watch-does = Escuta o que a central de segurança guarda e não roda nada: as conexões que o firewall recusa, o que o sudo faz, se o firewall é o que você pediu e quem pode assinar seus commits. Enquanto algo está fora do esperado, um escudo fica na modeline — laranja para o que você escolheu; na cor de perigo para o que ninguém escolheu, e essa vence sempre que houver um perigo. O ponteiro parado sobre ele diz tudo o que está fora, e o clique abre a central onde isso se resolve; em repouso não há nenhum. A página guarda as conexões recusadas há pouco e uma semana de sudo.
security-watch-mark-firewall-failed = O firewall não é o que você pediu
security-watch-mark-firewall-off = Seu firewall está desligado
security-watch-mark-file-key = Há chave privada num arquivo comum
security-watch-mark-tpm-signing = O TPM pode assinar seus commits, sem toque
security-watch-mark-sudo-open = O administrador não pede senha
security-watch-mark-shields = Toda conexão de entrada está sendo recusada
security-watch-mark-refused = { $count ->
    [one] Uma conexão de entrada recusada há pouco
   *[other] { $count } conexões de entrada recusadas há pouco
  }
security-watch-says-refused = { $count ->
    [one] Uma conexão recusada em { $iface }
   *[other] { $count } conexões recusadas em { $iface }
  }
security-watch-heading-now = Agora
security-watch-heading-blocked = Recusadas há pouco
security-watch-heading-admin = Administrador, nesta semana
security-watch-row-nothing = Nada fora do esperado
security-watch-row-firewall-failed = O firewall falhou
security-watch-row-firewall-off = O firewall está desligado
security-watch-row-sudo-open = Sem pedir senha até
security-watch-row-file-key = Chaves privadas em arquivos comuns
security-watch-row-tpm-signing = O TPM pode assinar seus commits
security-watch-row-shields = Toda conexão de entrada recusada
security-watch-row-refused = Recusadas no último quarto de hora
security-watch-row-journal-lost = O diário não pode ser lido
security-watch-sudo-used = Usado
security-watch-sudo-failed = Senha errada
security-watch-sudo-refused = Não permitido
notif-security-watch-network-public-summary = Uma rede nova em { $iface } é tratada como pública
notif-security-watch-network-public-body = Uma rede pública só deixa entrar o que suas regras para redes públicas permitem. Se você confia nela, trate-a como de casa.
