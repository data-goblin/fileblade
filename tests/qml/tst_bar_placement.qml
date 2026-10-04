import QtQuick
import QtTest
import "../../lib/BarPlacement.js" as BarPlacement

TestCase {
  name: "BarPlacement"

  function test_blades_sit_below_the_bar_unless_the_user_chose_beside() {
    compare(BarPlacement.normalize(undefined), "below")
    compare(BarPlacement.normalize(""), "below")
    compare(BarPlacement.normalize("Beside"), "beside")
    compare(BarPlacement.normalize("sideways"), "below")
  }

  function test_both_blades_start_under_a_top_bar() {
    compare(BarPlacement.insets("left", "top", 26, false, "below"), { top: 26, bottom: 0, left: 0, right: 0 })
    compare(BarPlacement.insets("right", "top", 26, false, "below"), { top: 26, bottom: 0, left: 0, right: 0 })
  }

  function test_both_blades_stop_above_a_bottom_bar() {
    compare(BarPlacement.insets("left", "bottom", 30, false, "below"), { top: 0, bottom: 30, left: 0, right: 0 })
    compare(BarPlacement.insets("right", "bottom", 30, false, "below"), { top: 0, bottom: 30, left: 0, right: 0 })
  }

  function test_a_side_bar_only_moves_the_blade_on_its_own_edge() {
    compare(BarPlacement.insets("left", "left", 28, false, "below"), { top: 0, bottom: 0, left: 28, right: 0 })
    compare(BarPlacement.insets("right", "left", 28, false, "below"), { top: 0, bottom: 0, left: 0, right: 0 })
    compare(BarPlacement.insets("right", "right", 28, false, "below"), { top: 0, bottom: 0, left: 0, right: 28 })
    compare(BarPlacement.insets("left", "right", 28, false, "below"), { top: 0, bottom: 0, left: 0, right: 0 })
  }

  function test_a_hidden_bar_gives_the_blade_the_full_height() {
    compare(BarPlacement.insets("left", "top", 26, true, "below"), { top: 0, bottom: 0, left: 0, right: 0 })
  }

  function test_beside_keeps_the_full_height_layout_and_reserves_before_the_bar() {
    compare(BarPlacement.insets("left", "top", 26, false, "beside"), { top: 0, bottom: 0, left: 0, right: 0 })
    verify(BarPlacement.reservesBeforeBar("beside"))
    verify(!BarPlacement.reservesBeforeBar("below"))
  }

  function test_an_unknown_bar_position_is_treated_as_a_top_bar() {
    compare(BarPlacement.insets("left", "diagonal", 26, false, "below"), { top: 26, bottom: 0, left: 0, right: 0 })
  }
}
