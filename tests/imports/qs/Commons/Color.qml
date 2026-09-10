pragma Singleton
import QtQuick
QtObject {
  property QtObject bar: QtObject { property color background: "#1a1b26"; property color text: "#eeeeee" }
  property QtObject popups: QtObject { property color background: "#1a1b26"; property color border: "#333333"; property color text: "#eeeeee" }
  property QtObject menu: QtObject { property color selectedBackground: "#14eeeeee" }
  property color accent: "#7aa2f7"
  property color muted: "#777777"
  property color urgent: "#ff6666"
  property color background: "#111111"
}
