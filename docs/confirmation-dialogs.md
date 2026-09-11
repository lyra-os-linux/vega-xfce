# Formulários e confirmações

A preferência **Confirmar ações administrativas** controla apenas confirmações
opcionais de ações que o usuário já definiu. Desativá-la não preenche nem envia
formulários automaticamente.

- `required_dialog` sempre aguarda uma resposta: configuração de backup, IPv4,
  descrição de snapshot, aprovação de propostas da IA, revisão de rollback e
  confiança em chaves de repositório.
- `confirm_dialog` pode dispensar uma confirmação simples, mas sempre apresenta
  diálogos com `extra_child`. Essa proteção atende também ao diálogo de Wi-Fi:
  redes protegidas coletam senha; redes abertas e desconexão seguem a preferência.
- Cancelar ou fechar continua interrompendo a ação. Validações de campos e
  autorização do backend permanecem nos fluxos existentes.
- Adicionar repositório usa um formulário na própria página Software. Nome e
  URL continuam obrigatórios com a preferência ligada ou desligada.

A descrição da preferência informa a regra em português, inglês e espanhol.
Propostas da IA sempre exigem aprovação explícita, conforme a política de
[privacidade do Assistente](ai-privacidade.md).

## Validação da issue Vega XFCE #4

`application::dialog_tests::native_dialog_flows` instancia os widgets GTK reais,
aciona os callbacks da aplicação e verifica os parâmetros recebidos por um
serviço D-Bus simulado. O teste cria um barramento privado sem ativação de
serviços e diretórios XDG temporários. Nenhum pedido alcança o vegad do host.
O serviço registra as mutações e devolve erro deliberadamente; não instala
pacotes, altera rede ou cria backups/snapshots.

Com a preferência ligada e desligada, cobre:

- backup: cancelar, fechar, recusar campos vazios e enviar campos preenchidos,
  caminhos separados por vírgula, UUID e frequência semanal;
- snapshot, IPv4 e Wi-Fi: aguardar entrada e enviar os dados preenchidos;
- repositório: campos vazios não enviam pedido; nome e URL preenchidos chegam
  ao backend sem espaços nas extremidades;
- IA: instalar, remover e limpar cache, cada ação aprovada e rejeitada;
- rollback: mostrar as diferenças retornadas pelo backend antes da decisão;
- repositório com chave verificável ou sem chave: exibir os detalhes e aguardar
  aprovação, com cancelamento sem pedido de confiança;
- confirmação simples: respeitar a preferência.

Execução em CI, com Xvfb:

```sh
xvfb-run -a cargo test --workspace --locked native_dialog_flows -- --ignored --test-threads=1
```

A suíte usa dados e serviços simulados, sem executar operações reais em VM/RPM.

Em 11/09/2026, a base `6f306145955d3ba870ea920d517a191f7a1c5d11` passou
pela etapa com confirmações ligadas, mas falhou ao aguardar o formulário de
backup com a preferência desligada. A correção passou nas duas etapas em
Mutter/Wayland isolado. Também passaram 37 testes Rust padrão, quatro testes
Python de tradução, fmt, Clippy estrito, contrato D-Bus e parse do spec RPM.

GTK emitiu avisos de largura mínima de rótulos tanto na base quanto na correção.
Este ensaio verifica interação e argumentos D-Bus, não a aparência completa,
acessibilidade, autorização real do daemon ou uma sessão XFCE completa.
