import QtQuick
import QtQuick.Effects
import qs.Commons
import "WelcomePlan.js" as WelcomePlan
import "../../theme"

FocusScope {
  id: module

  property var context: null

  readonly property string title: "Welcome"
  readonly property var shortcuts: [
    { title: "Welcome", items: [{ shortcut: "Enter", text: "Install the example extensions" }, { shortcut: "Esc", text: "Close and do not show again" }] }
  ]
  readonly property color paneBackground: Qt.lighter(Color.background, 1.035)
  readonly property var files: context ? context.service("files") : null
  readonly property var welcome: files ? files.welcome : null
  readonly property bool installing: !!welcome && welcome.installing
  readonly property bool placing: !!welcome && welcome.placingActive
  readonly property bool placementFailed: !!welcome && welcome.placementFailed
  readonly property string installError: welcome ? String(welcome.error || "") : ""
  readonly property int missingCount: welcome ? welcome.missing().length : 0
  readonly property bool installable: (missingCount > 0 || placementFailed) && !installing && !placing
  readonly property real nameColumnWidth: Math.ceil(nameMetrics.width)

  TextMetrics {
    id: nameMetrics
    font.family: Style.font.family
    font.pixelSize: Typography.body
    text: WelcomePlan.EXTENSIONS.map(function(item) { return item.name }).reduce(function(longest, name) { return name.length > longest.length ? name : longest }, "")
  }

  function takeFocus(part) { module.forceActiveFocus() }
  function has(moduleId) { return !!welcome && welcome.registryHas(moduleId) }
  function install() { if (welcome) welcome.install() }
  function dismiss() { if (welcome) welcome.dismiss() }

  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { module.install(); event.accepted = true }
    else if (event.key === Qt.Key_Escape) { module.dismiss(); event.accepted = true }
  }

  Rectangle { anchors.fill: parent; color: module.paneBackground }

  Column {
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.margins: Style.space(12)
    spacing: Style.space(10)

    Text {
      textFormat: Text.PlainText
      width: parent.width
      text: WelcomePlan.HEADING
      color: Color.bar.text
      wrapMode: Text.WordWrap
      font.family: Style.font.family
      font.pixelSize: Typography.body
      font.weight: Font.DemiBold
    }

    Text {
      textFormat: Text.PlainText
      width: parent.width
      text: WelcomePlan.BODY
      color: Color.muted
      wrapMode: Text.WordWrap
      font.family: Style.font.family
      font.pixelSize: Typography.body
    }

    Column {
      width: parent.width
      spacing: Style.space(4)
      Repeater {
        model: WelcomePlan.EXTENSIONS
        delegate: Item {
          id: extensionRow
          required property var modelData
          width: parent ? parent.width : 0
          height: Style.space(18)
          activeFocusOnTab: true
          Accessible.role: Accessible.Link
          Accessible.name: modelData.name + " source code on GitHub"
          function openSource() { Qt.openUrlExternally(String(modelData.url).replace(/\.git$/, "")) }
          Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
              openSource()
              event.accepted = true
            }
          }
          Item {
            id: glyphSlot
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: Style.space(14)
            height: parent.height
            Text {
              textFormat: Text.PlainText
              anchors.centerIn: parent
              visible: !extensionRow.modelData.image
              text: extensionRow.modelData.glyph
              color: Color.accent
              font.family: Style.font.family
              font.pixelSize: Typography.body
            }
            Image {
              id: logo
              anchors.centerIn: parent
              width: Style.space(11)
              height: width
              visible: false
              source: extensionRow.modelData.image ? Qt.resolvedUrl(extensionRow.modelData.image) : ""
              sourceSize: Qt.size(64, 64)
              smooth: true
            }
            MultiEffect {
              source: logo
              anchors.fill: logo
              visible: !!extensionRow.modelData.image && logo.status === Image.Ready
              colorization: 1
              colorizationColor: Color.accent
            }
          }
          Text {
            id: nameText
            textFormat: Text.PlainText
            anchors.left: glyphSlot.right
            anchors.leftMargin: Style.space(8)
            anchors.verticalCenter: parent.verticalCenter
            width: module.nameColumnWidth
            text: extensionRow.modelData.name
            color: sourcePointer.containsMouse || extensionRow.activeFocus ? Color.accent : Color.bar.text
            font.family: Style.font.family
            font.pixelSize: Typography.body
            font.underline: true
          }
          Text {
            textFormat: Text.PlainText
            anchors.left: nameText.right
            anchors.leftMargin: Style.space(10)
            anchors.verticalCenter: parent.verticalCenter
            visible: module.has(extensionRow.modelData.module)
            text: "installed"
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Typography.body
          }
          MouseArea {
            id: sourcePointer
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: extensionRow.openSource()
          }
        }
      }
    }

    Rectangle {
      width: installContent.implicitWidth + Style.space(24)
      height: Style.space(26)
      radius: 0
      color: !module.installable ? Util.alpha(Color.muted, 0.25) : (installPointer.containsMouse ? Qt.lighter(Color.accent, 1.1) : Color.accent)
      Row {
        id: installContent
        anchors.centerIn: parent
        spacing: Style.space(8)
        Item {
          id: busyIcon
          visible: module.installing || module.placing
          anchors.verticalCenter: parent.verticalCenter
          width: Style.space(18)
          height: width
          Image {
            id: busyImage
            anchors.fill: parent
            source: "../../assets/fileblade-icon.svg"
            sourceSize: Qt.size(64, 64)
            visible: false
            smooth: true
          }
          MultiEffect {
            source: busyImage
            anchors.fill: parent
            colorization: 1
            colorizationColor: Color.accent
          }
          SequentialAnimation on opacity {
            running: (module.installing || module.placing) && !!module.files && module.files.bladeHost.animateBlades
            loops: Animation.Infinite
            onStopped: busyIcon.opacity = 1
            NumberAnimation { from: 1; to: 0.25; duration: 450; easing.type: Easing.InOutSine }
            NumberAnimation { from: 0.25; to: 1; duration: 450; easing.type: Easing.InOutSine }
          }
        }
        Text {
          id: installLabel
          textFormat: Text.PlainText
          anchors.verticalCenter: parent.verticalCenter
          text: module.installing && module.welcome ? "Installing " + (module.welcome.installed + 1) + " of " + module.welcome.queue.length + "…"
            : module.placing ? "Adding tabs…"
            : (module.missingCount === 0 && !module.placementFailed ? "Installed" : "Install")
          color: module.installing || module.placing ? Color.bar.text : (module.installable ? Color.background : Color.muted)
          font.family: Style.font.family
          font.pixelSize: Typography.body
          font.weight: Font.DemiBold
        }
      }
      MouseArea {
        id: installPointer
        anchors.fill: parent
        hoverEnabled: true
        enabled: module.installable
        cursorShape: Qt.PointingHandCursor
        onClicked: module.install()
      }
    }

    Text {
      textFormat: Text.PlainText
      width: parent.width
      text: WelcomePlan.SOURCE_NOTICE
      color: module.files ? module.files.themedFolderColor("yellow", "#e5c07b") : Color.accent
      wrapMode: Text.WordWrap
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
    }

    Text {
      textFormat: Text.PlainText
      width: parent.width
      visible: module.installError !== ""
      text: module.installError
      color: Color.urgent
      wrapMode: Text.WordWrap
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
    }
  }

  Text {
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.bottom: parent.bottom
    anchors.margins: Style.space(12)
    text: WelcomePlan.DISMISS
    color: dismissPointer.containsMouse ? Color.bar.text : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.caption
    font.underline: true
    MouseArea {
      id: dismissPointer
      anchors.fill: parent
      anchors.margins: -Style.space(4)
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: module.dismiss()
    }
  }
}
