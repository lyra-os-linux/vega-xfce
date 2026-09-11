# Privacidade do Assistente nativo

- Chaves de API são armazenadas exclusivamente no Secret Service pelo
  `secret-tool`; o Vega não mantém uma cópia em seus arquivos.
- Configurações, uso diário, histórico e auditoria usam arquivos privados com
  permissão `0600` dentro do diretório de dados do usuário.
- A auditoria redige chaves conhecidas, e-mails, endereços IP e caminhos de
  diretórios pessoais antes da gravação.
- Erros HTTP são redigidos antes de aparecerem na interface ou auditoria.
- Tools de mutação geram uma proposta visível e só chamam o `vegad` depois de
  confirmação explícita. Cancelar é sempre a resposta padrão.
- Instalações AUR são recusadas pelo Assistente e permanecem na tela Software,
  onde o PKGBUILD precisa ser revisado.
- As mensagens da conversa são enviadas ao provedor selecionado. O usuário
  deve consultar a política de privacidade do respectivo provedor.
- Resultados de tools de leitura voltam ao agente delimitados como dados externos
  não confiáveis, reduzindo o risco de conteúdo de pacote ou sistema ser tratado
  como instrução.
- O loop do agente possui limite configurável entre uma e vinte etapas; o limite
  diário é contabilizado por mensagem do usuário, não por etapa interna.

- Limpar conversa independe da preferência de persistência e remove também as
  cópias antigas de mensagens/nomes de anexos no audit. Registros operacionais
  são mantidos separadamente; veja a [política de retenção](../ai-privacidade.md).
