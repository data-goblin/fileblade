import QtQuick
import QtQuick.Effects
import Quickshell
import qs.Commons
import "../lib/FileIcons.js" as FileIcons
import "../theme"
import qs.Commons as Commons

Item {
  id: root

  property string iconName: ""
  property string trustedIconSource: ""
  property bool monochrome: false
  property string monochromeMask: "alpha"
  property color iconColor: fallbackColor
  property string fallbackGlyph: ""
  property color fallbackColor: Commons.Color.accent
  property real fallbackSize: Typography.body
  property real iconSize: Style.space(18)
  property string desktopId: ""
  property var applicationDescriptor: null
  property var applicationOverride: null
  property var rejectedSources: []
  readonly property var desktopEntry: desktopId === "" ? null : DesktopEntries.applications.values.find(function(entry) {
    return entry.id === root.desktopId.replace(/\.desktop$/, "")
  }) || null
  readonly property var applicationIcon: {
    if (desktopId === "" && applicationDescriptor === null) return null
    var entry = Object.assign({}, applicationDescriptor || { icon: iconName, glyph: fallbackGlyph })
    var override = applicationOverride
    var desktop = desktopEntry
    var resolved = FileIcons.resolveApplication(entry, override, desktop, Quickshell.iconPath)
    var glyph = resolved.glyph
    for (var step = 0; step < 4 && rejectedSources.indexOf(resolved.icon_source) >= 0; ++step) {
      if (step === 0) override = null
      else if (step === 1) desktop = null
      else if (step === 2) entry.icon_source = ""
      else entry.icon = ""
      resolved = FileIcons.resolveApplication(entry, override, desktop, Quickshell.iconPath)
    }
    return { icon: resolved.icon, icon_source: resolved.icon_source, glyph: glyph }
  }
  onApplicationDescriptorChanged: rejectedSources = []
  onApplicationOverrideChanged: rejectedSources = []
  onDesktopEntryChanged: rejectedSources = []
  onDesktopIdChanged: rejectedSources = []
  onIconNameChanged: rejectedSources = []
  onFallbackGlyphChanged: rejectedSources = []
  readonly property string resolvedSource: {
    if (applicationIcon !== null) return applicationIcon.icon_source
    if (trustedIconSource !== "") return trustedIconSource
    var safeName = FileIcons.safeThemeIconName(iconName)
    return safeName ? Quickshell.iconPath(safeName, true) : ""
  }

  width: iconSize
  height: iconSize

  Text {
    visible: text !== "" && icon.status !== Image.Ready
    anchors.centerIn: parent
    textFormat: Text.PlainText
    text: root.applicationIcon === null ? root.fallbackGlyph : root.applicationIcon.glyph
    color: root.fallbackColor
    font.family: Style.font.family
    font.pixelSize: root.fallbackSize
  }

  Image {
    id: icon
    visible: !root.monochrome && root.resolvedSource !== "" && icon.status === Image.Ready
    anchors.fill: parent
    fillMode: Image.PreserveAspectFit
    sourceSize.width: Math.min(128, Math.max(1, width * Screen.devicePixelRatio))
    sourceSize.height: Math.min(128, Math.max(1, height * Screen.devicePixelRatio))
    source: root.resolvedSource
    asynchronous: true
    onStatusChanged: {
      if (status !== Image.Error || root.applicationIcon === null) return
      var failedSource = String(source)
      Qt.callLater(function() {
        if (root.resolvedSource === failedSource && root.rejectedSources.length < 4
            && root.rejectedSources.indexOf(failedSource) < 0)
          root.rejectedSources = root.rejectedSources.concat([failedSource])
      })
    }
  }

  readonly property bool maskedByLuminance: monochromeMask === "luminance" || monochromeMask === "dark"

  MultiEffect {
    anchors.fill: icon
    source: icon
    visible: root.monochrome && !root.maskedByLuminance
      && root.resolvedSource !== "" && icon.status === Image.Ready
    colorization: 1
    colorizationColor: root.iconColor
  }

  ShaderEffect {
    anchors.fill: icon
    visible: root.monochrome && root.maskedByLuminance
      && root.resolvedSource !== "" && icon.status === Image.Ready
    property var source: icon
    property color glyphColor: root.iconColor
    fragmentShader: Qt.resolvedUrl(root.monochromeMask === "dark"
      ? "shaders/DarkGlyph.frag.qsb"
      : "shaders/LuminanceGlyph.frag.qsb")
  }
}
