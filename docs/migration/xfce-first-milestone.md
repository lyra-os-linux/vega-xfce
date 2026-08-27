# Vega XFCE: primeiro marco

O primeiro marco mantém a interface existente em GTK4/libadwaita e migra as
fronteiras que pertencem ao ambiente desktop. Isso entrega uma versão útil
mais cedo sem duplicar o daemon privilegiado nem o contrato D-Bus.

## Decisões

- pacote e binário próprios: `vega-xfce`;
- ID de aplicação próprio: `org.lyraos.Vega.Xfce`;
- `vegad` e `org.lyraos.Vega1.*` continuam compartilhados;
- o wallpaper usa `xfconf-query` no canal `xfce4-desktop` e atualiza todas as
  propriedades de monitor/workspace já criadas pelo `xfdesktop`;
- o backend GSettings permanece como fallback durante desenvolvimento;
- libadwaita é uma dependência transitória da interface, não uma dependência
  da sessão GNOME. Sua remoção será avaliada página a página após as
  integrações funcionais do XFCE.

## Próximos cortes

1. aparência e ícones via canal `xsettings`;
2. bloqueio de tela e tempo ocioso via componentes instalados no flavor;
3. auditoria das páginas que duplicam ferramentas nativas do XFCE;
4. teste de integração numa imagem experimental do LyraOS XFCE.
