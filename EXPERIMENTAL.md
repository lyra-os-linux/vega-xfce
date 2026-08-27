# Vega XFCE experimental

O objetivo deste repositório é adaptar a interface GTK do Vega à experiência
XFCE mantendo o daemon `vegad`, o contrato D-Bus e as fronteiras de privilégio
compartilhadas. O fork não deve duplicar operações privilegiadas nem criar um
daemon específico do desktop.

Antes do primeiro pacote serão definidos os contratos de integração com o
painel, configurações, notificações, sessão e aparência do XFCE.
