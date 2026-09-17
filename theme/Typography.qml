pragma Singleton
import QtQuick
import qs.Commons
import "../lib/Typography.js" as Typography

QtObject {
  id: root

  readonly property int minimumPercent: Typography.MINIMUM_PERCENT
  readonly property int maximumPercent: Typography.MAXIMUM_PERCENT
  readonly property int percentStep: Typography.PERCENT_STEP

  property real scale: 1.0

  readonly property int percent: Typography.percentFromScale(scale)

  function clamp(value) { return Typography.clamp(value) }
  function scaleFromPercent(value) { return Typography.scaleFromPercent(value) }
  function px(size) { return Typography.px(size, scale, Style.font.body) }

  readonly property int caption: px(Style.font.caption)
  readonly property int bodySmall: px(Style.font.bodySmall)
  readonly property int body: px(Style.font.body)
  readonly property int title: px(Style.font.title)
}
