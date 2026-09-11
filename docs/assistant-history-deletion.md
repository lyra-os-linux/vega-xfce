# Exclusão do histórico — Vega XFCE #3

A opção de persistência controla leituras e novas gravações. Uma solicitação
explícita de apagar a conversa remove `ai-history.json` independentemente dessa
opção. O arquivo inclui as cópias dos anexos; os arquivos originais permanecem.

A limpeza também retira os registros legados `user_message`, `assistant_message`
e `user_attachment` de `ai-audit.jsonl`. Esses tipos deixam de ser gravados.
Eventos operacionais têm retenção separada, descrita em
[Privacidade do assistente](ai-privacidade.md#limpar-conversa-e-retenção-local).
A reescrita usa um arquivo temporário privado e substituição por rename; as duas
exclusões não constituem uma transação atômica conjunta. Uma falha pode ocorrer
depois de remover parte das cópias. A interface informa o erro e permite repetir.

A operação de disco roda fora da thread GTK. Conversa, rascunho e anexos pendentes
só são removidos da interface após sucesso. Envio, importação e limpeza não
podem se sobrepor na mesma página. Não há sincronização com escritores externos.

## Verificação reproduzível

```sh
cargo test --workspace --locked
xvfb-run -a dbus-run-session -- cargo test --workspace --locked native_history_ui -- --ignored --test-threads=1
python3 -m unittest discover -s tests -v
```

Os testes usam subprocessos com diretórios XDG temporários definidos antes da
inicialização do GLib. A suíte de armazenamento cobre persistência real,
reativação sem reaparecimento, anexos, retenção operacional, exclusão repetida,
dados ausentes, log inválido, erros de remoção e links simbólicos. O teste GTK
exercita widgets reais, preservação da apresentação em erro e limpeza completa
após corrigir o erro. Não usa provedor de IA nem transações de pacotes.

Em 11/09/2026, o cenário salvar → desativar → limpar → reativar falhou na
base `0e64ec9c2fa2454c74e85da414af42c79b24d2e6`: a função retornou sucesso,
mas a conversa antiga reapareceu ao reativar a persistência. Com a correção,
passaram os 37 testes Rust padrão (incluindo sete cenários de armazenamento
em subprocessos), o teste GTK em compositor Wayland isolado e os quatro testes
Python de tradução. Fmt, Clippy estrito, contrato D-Bus e parse do spec passaram.
Não houve sessão XFCE completa, provedor externo ou qualificação de RPM/ISO.
