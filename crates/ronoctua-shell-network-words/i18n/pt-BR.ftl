# The texts `ronoctua-shell-network-words` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Energia — a bateria, o carregador, o perfil e o sono (LEADER s p)

echo-network-refused = A rede recusou: { $why }
# A TABELA, de `assets/tor-options.nix` pelo gerador. Uma chave que ninguém
# escreveu desenha como `[tor-opt-…]` na tela, e é por isso que o
# `tor-options-have-text` as conta.
tor-opt-socks-port = endereço SOCKS
tor-opt-cookie-authentication = autenticação por cookie
tor-opt-cookie-file = onde fica o cookie
tor-opt-cookie-group-readable = o grupo tor pode ler o cookie
tor-opt-exit-nodes = sair por
tor-opt-entry-nodes = entrar por
tor-opt-exclude-nodes = nunca usar
tor-opt-exclude-exit-nodes = nunca sair por
tor-opt-strict-nodes = tratar essas listas como regra, não preferência
tor-opt-circuit-lifetime = usar um circuito por
tor-opt-use-bridges = usar pontes
tor-opt-bridge-lines = pontes
tor-opt-bridge-transport = transporte
tor-opt-firewall-ports = portas que esta rede deixa sair
tor-opt-fascist-firewall = só sair pelas portas 80 e 443
tor-opt-onion-directory = onde a chave onion é guardada
tor-opt-onion-ports = portas que este serviço onion atende
tor-opt-onion-single-hop = um salto só — rápido, e não anônimo
tor-opt-safe-socks = recusar pedidos SOCKS que vazariam o nome
tor-opt-new-circuit-period = considerar um circuito novo a cada
tor-opt-circuit-build-timeout = desistir de montar um circuito após
tor-opt-entry-guards = quantas guardas de entrada
tor-opt-use-entry-guards = continuar entrando pelos mesmos relés
tor-opt-distinct-subnets = manter os relés de um circuito em redes diferentes
tor-opt-ipv6 = usar IPv6
tor-opt-prefer-ipv6 = preferir IPv6
tor-opt-connection-padding = tráfego de cobertura
tor-opt-reduced-padding = menos tráfego de cobertura
tor-opt-avoid-disk-writes = escrever no disco o mínimo possível
tor-opt-dormant-timeout = ficar ocioso após
tor-opt-dormant-on-start = começar ocioso
tor-opt-socks-proxy = alcançar a rede por um proxy SOCKS
tor-opt-https-proxy = alcançar a rede por um proxy HTTPS
tor-opt-log-level = o que o Tor escreve no log
tor-unit-seconds = segundos
tor-unit-guards = guardas
tor-word-none = nenhum
tor-word-obfs4 = obfs4
tor-word-snowflake = snowflake
tor-word-meek-lite = meek
tor-word-auto = como o Tor decidir
tor-word-0 = desligado
tor-word-1 = ligado
tor-word-notice-syslog = o de sempre
tor-word-info-syslog = mais
tor-word-debug-syslog = tudo
echo-network-login = abra { $url } para terminar de entrar

## file info editing — everything the info panel can change

tunnel-refused-no-daemon = o daemon dele não está rodando
tunnel-refused-authority = o socket dele não é desta sessão
tunnel-refused-unit = o serviço dele não quis subir nem descer
tunnel-refused-not-written = o arquivo não pôde ser gravado
tunnel-refused-config = o Tor não leu a configuração
tunnel-refused-daemon = o daemon disse não
tunnel-refused-other = não funcionou
echo-tunnel-refused-line = O Tor não leu esta linha: { $line }
