import QtQuick
import Quickshell.Io
import qs.Commons
import qs.Ui
import "../blades"

BarWidget {
  id: widget
  moduleName: "data-goblin.fileblade"
  readonly property var service: bar && bar.shell ? bar.shell.serviceFor(moduleName) : null
  readonly property var host: service ? service.bladeHost : null
  property string requestedModule: ""
  readonly property string moduleId: requestedModule || String(setting("module", "notes"))
  readonly property bool opened: panel.open
  readonly property bool popoutSwitchClosing: panel.popoutSwitchClosing
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function openModule(module) {
    if (!host || !host.registry.module(module)) return "invalid-module"
    requestedModule = module
    panel.open = true
    return "opened"
  }
  function close() { panel.open = false }
  function closeForPopoutSwitch() { close() }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: widget.bar
    text: "󰈔"
    active: widget.opened
    tooltipText: content.title || "FileBlade"
    onPressed: function(mouseButton) {
      if (mouseButton !== Qt.LeftButton) return
      if (widget.opened) widget.close()
      else widget.openModule(widget.moduleId)
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    bar: widget.bar
    owner: widget
    contentWidth: panel.fittedContentWidth(Style.space(Math.max(240, Math.min(800, Number(widget.setting("width", 460)) || 460))))
    contentHeight: panel.cappedContentHeight(Style.space(Math.max(160, Math.min(1000, Number(widget.setting("height", 600)) || 600))))
    focusTarget: content
    BladePopout {
      id: content
      anchors.fill: parent
      host: widget.host
      shell: widget.bar ? widget.bar.shell : null
      services: widget.service ? widget.service.services : ({})
      screen: panel.screen
      moduleId: widget.moduleId
      opened: panel.open
      onCloseRequested: widget.close()
    }
  }

  IpcHandler {
    target: "data-goblin.fileblade.popout"
    function open(module: string): string { return widget.openModule(module) }
    function close(): string { widget.close(); return "closed" }
    function status(): string {
      return JSON.stringify({ opened: widget.opened, module: widget.moduleId, loaded: !!content.moduleItem, notice: content.notice })
    }
  }
}
