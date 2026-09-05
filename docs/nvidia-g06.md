A instalação de drivers NVIDIA e firmware pelo Vega foi removida.

As interfaces GNOME e XFCE exibem o inventário de hardware e o estado do
firmware informado pelo fwupd, sem cartões de instalação. A monitoração de
uso da GPU continua disponível.

O vegad não adiciona repositórios NVIDIA, instala a pilha G06, troca drivers
nem instala firmware opcional por esses fluxos. Os métodos D-Bus v1
`Software.InstallNvidia`, `Software.InstallNonFreeFirmware` e
`Hardware.SwitchNvidiaDriver` permanecem apenas para compatibilidade: retornam
`org.freedesktop.DBus.Error.NotSupported` antes de autenticação, execução de
comandos ou criação de transação. As ações Polkit correspondentes foram
removidas. As consultas legadas de diagnóstico continuam disponíveis.

A proteção de suspensão já existente para NVIDIA 580.159.03 em notebooks
híbridos continua sendo reconciliada na inicialização do daemon. Remover os
instaladores não altera os drivers instalados nem desativa essa proteção.
O comportamento dos módulos gerais de pacotes, kernel e recuperação permanece
independente desses fluxos removidos.

Para distribuir a mudança, atualizar os frontends e o vegad de forma coordenada.
Um frontend novo não chama os instaladores antigos; um frontend antigo conectado
ao daemon novo recebe a recusa explícita. O contrato v1 conserva suas assinaturas
e documenta os três métodos como descontinuados.
