# Privacidade do Assistente de IA

Este documento descreve exatamente o que o assistente de IA do Vega envia para provedores externos, e o que ele nunca envia. Cobre a issue [#34](https://github.com/lyra-os-linux/vega/issues/34).

## Resumo

O Vega **não tem servidor próprio** para o assistente — cada mensagem sua é enviada diretamente do seu computador para a API do provedor que você escolheu (Anthropic, OpenAI ou Google), usando a sua própria chave de API. O Vega não vê, armazena ou retransmite essas mensagens em nenhum servidor intermediário.

## O que é enviado a cada mensagem

Toda vez que você envia uma mensagem no chat, o seguinte é enviado para a API do provedor ativo:

- **O texto da sua mensagem** e o histórico da conversa atual, armazenado
  localmente no diretório privado de dados do Vega até você usar “Limpar
  conversa”.
- **Os resultados das ferramentas de leitura** que o modelo decidir usar para responder sua pergunta. Hoje isso pode incluir:
  - Busca de pacotes, lista de instalados, lista de atualizações disponíveis
  - Hardware (CPU, GPU, RAM) e status de firmware
  - Uso de disco e lista de volumes de armazenamento (caminhos, pontos de montagem, tamanho)
  - Kernels instalados e disponíveis
  - **Interfaces de rede** (nome, tipo, IP, MAC, estado) e **status do firewall** (zona ativa, serviços permitidos)
  - Data/hora, fuso horário, locale e layout de teclado configurados
  - Métricas do sistema (CPU, memória, swap, disco, rede) e lista dos processos mais pesados
  - **Lista de usuários** do sistema (nome de conta, se é administrador)
  - Lista de serviços systemd gerenciados (nome, status) e lista de snapshots existentes
  - **Log do systemd de uma unidade específica** — só quando você pedir explicitamente por log/diagnóstico; a ferramenta limita a no máximo 100 linhas por consulta, mesmo que o modelo peça mais
- **Os parâmetros de ações de mutação propostas** (ex.: nome e origem de um pacote a instalar/remover, nome de um serviço, caminho de um volume) — só depois que você já confirmou a ação no diálogo, como parte do resultado da ferramenta que volta pro modelo continuar a conversa.

## O que **não** é enviado

- Sua chave de API nunca é enviada a nenhum servidor além do próprio provedor
  escolhido; ela fica no Secret Service/keyring do sistema.
- Nenhuma ação de mutação é executada sem confirmação explícita sua no diálogo — o modelo nunca autoriza uma ação sozinho a partir de texto livre.
- Módulos ainda fora do escopo do assistente não têm ferramentas disponíveis
  para o modelo, mesmo quando possuem controles próprios na interface.
- O log de auditoria local (`ai-audit.jsonl`, na pasta de dados do usuário) nunca sai do seu computador — registra ações do sistema e erros redigidos para consulta posterior. Mensagens e nomes de anexos não são mais duplicados nesse log.

## Mitigação contra prompt injection

Conteúdo retornado pelas ferramentas de leitura (por exemplo, a descrição de um pacote) é dado externo, não confiável — em tese, alguém poderia publicar um pacote com uma descrição tentando instruir o modelo a fazer algo indevido. Três camadas de proteção:

1. O prompt de sistema instrui o modelo explicitamente a nunca tratar o conteúdo das ferramentas como instrução, só a mensagem do usuário.
2. Estruturalmente, todo resultado de ferramenta de leitura é envolvido num delimitador explícito (`<dado_nao_confiavel origem="tool:...">...</dado_nao_confiavel>`) antes de entrar na conversa enviada ao provedor — não depende só da instrução em texto livre, é parte da própria mensagem.
3. Nenhuma ferramenta de mutação executa sem que o diálogo de confirmação mostre o nome da ação e os parâmetros *reais e estruturados* — nunca uma descrição gerada livremente pelo modelo. Mesmo que o modelo seja induzido a *propor* algo malicioso, você vê exatamente o que seria executado antes de confirmar.

## Limites e auditoria

- **Limite de mensagens por dia** e **limite de rodadas de tool-call por mensagem**, ambos configuráveis na tela do Assistente — protegem contra loops do agente ou uso excessivo por bug, não contra custo indevido pro projeto (você usa sua própria chave, então o custo de API é seu).
- O log de auditoria local passa por uma redação básica antes de gravar: e-mails e padrões conhecidos de chave de API (ex. `sk-...`, `AIza...`) são substituídos por `[redigido]` antes de persistir, caso apareçam por acidente numa mensagem ou resultado de ferramenta.

## Política de cada provedor

Cada provedor tem sua própria política de retenção e uso de dados enviados via API — o Vega não tem controle sobre isso além de qual provedor você escolhe usar:

- **Anthropic (Claude)**: https://www.anthropic.com/legal/privacy
- **OpenAI (ChatGPT)**: https://openai.com/policies/privacy-policy/
- **Google (Gemini)**: https://policies.google.com/privacy

## Ferramentas de mutação disponíveis

Todas seguem o mesmo fluxo: o modelo só propõe, você confirma no diálogo (vendo o nome da ação e os parâmetros reais), e só então a ação real é executada.

- **Software**: instalar/remover pacote, limpar cache e atualizar tudo
- **Snapshots**: criar, reverter (`rollback` — ação de alto risco, o diálogo avisa explicitamente), apagar, definir política de retenção
- **Serviços**: habilitar/desabilitar no boot, iniciar/parar, reiniciar — o diálogo não impede desabilitar um serviço essencial (ex. rede, ssh); a confirmação humana é a única barreira, então preste atenção no nome do serviço antes de confirmar
- **Armazenamento**: montar/desmontar volume — desmontar um volume em uso pode causar perda de dados; o Vega não verifica processos abertos antes de propor, então confirme com cuidado

`SetRepoEnabled` (habilitar/desabilitar repositório) permanece fora das
ferramentas da IA por alterar configuração persistente. A ação existe somente
na tela Software e exige confirmação explícita do usuário.

## Cobertura de leitura

O assistente cobre leitura de: Software, Hardware, Disco/Armazenamento, Kernel, Rede, Firewall (status), Data/Hora, Monitor de Sistema, Usuários, Serviços, Snapshots e Log do systemd (com o limite de 100 linhas citado acima). Ainda sem cobertura de leitura: Backup, Bluetooth, Proxy, Boot/Bootloader — perguntas sobre esses módulos não têm como ser respondidas com dado real ainda.

## Limpar conversa e retenção local

“Limpar conversa” apaga o histórico local mesmo quando salvar histórico está
desativado. Os anexos enviados ficam embutidos nesse arquivo e suas cópias são
apagadas junto dele. Também limpa rascunhos e anexos pendentes da interface;
os arquivos originais escolhidos pelo usuário permanecem no lugar.

Cópias antigas de mensagens e nomes de anexos no `ai-audit.jsonl` são removidas
na mesma ação. Registros operacionais (ações propostas/aprovadas/recusadas,
leituras, rejeições de ferramentas e erros redigidos) permanecem separados,
sem prazo automático de expiração nesta versão. Limpar conversa não redefine
limites de uso, configurações nem credenciais do keyring.

A ação fica indisponível enquanto uma resposta, ferramenta ou importação de
anexo está em andamento, evitando repovoar a conversa logo após a limpeza.
Em caso de falha, o Vega informa o erro e preserva a conversa na interface.
A exclusão local não apaga dados já enviados ao provedor, cópias de segurança
ou snapshots; não equivale a sobrescrita segura do armazenamento.
