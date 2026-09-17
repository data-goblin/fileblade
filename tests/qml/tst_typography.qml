import QtQuick
import QtTest
import "../../lib/Typography.js" as Typography

TestCase {
  name: "TypographyScale"

  function test_default_scale_leaves_shell_tokens_untouched() {
    compare(Typography.px(10, 1.0), 10)
    compare(Typography.px(11, 1.0), 11)
    compare(Typography.px(12, 1.0), 12)
    compare(Typography.px(14, 1.0), 14)
  }

  function test_shell_tokens_grow_with_the_chosen_scale() {
    compare(Typography.px(10, 1.25), 13)
    compare(Typography.px(11, 1.25), 14)
    compare(Typography.px(12, 1.25), 15)
    compare(Typography.px(14, 1.25), 18)
    compare(Typography.px(10, 1.5), 15)
    compare(Typography.px(11, 1.5), 17)
    compare(Typography.px(12, 1.5), 18)
    compare(Typography.px(14, 1.5), 21)
  }

  function test_scale_below_the_minimum_settles_on_the_minimum() {
    compare(Typography.clamp(0.5), 0.75)
    compare(Typography.clamp(0.74), 0.75)
    compare(Typography.px(12, 0.5), 9)
  }

  function test_scale_above_the_maximum_settles_on_the_maximum() {
    compare(Typography.clamp(2.5), 2.0)
    compare(Typography.clamp(64), 2.0)
    compare(Typography.px(12, 64), 24)
  }

  function test_unusable_scale_falls_back_to_unscaled() {
    compare(Typography.clamp(undefined), 1.0)
    compare(Typography.clamp(null), 1.0)
    compare(Typography.clamp("large"), 1.0)
    compare(Typography.clamp(0), 1.0)
    compare(Typography.clamp(-3), 1.0)
    compare(Typography.px(12, "large"), 12)
  }

  function test_a_mistyped_theme_token_degrades_to_readable_body_text() {
    compare(Typography.px(undefined, 1.0, 12), 12)
    compare(Typography.px(undefined, 1.25, 12), 15)
    compare(Typography.px(0, 1.5, 12), 18)
    compare(Typography.px(-4, 1.5, 12), 18)
  }

  function test_unusable_token_size_without_a_usable_fallback_stays_renderable() {
    compare(Typography.px(undefined, 1.5), 1)
    compare(Typography.px(0, 1.5), 1)
    compare(Typography.px(-4, 1.5), 1)
    compare(Typography.px(undefined, 1.5, "body"), 1)
    compare(Typography.px(1, 0.75), 1)
  }

  function test_a_typed_percentage_becomes_its_scale() {
    compare(Typography.scaleFromPercent(100), 1.0)
    compare(Typography.scaleFromPercent(110), 1.1)
    compare(Typography.scaleFromPercent(133), 1.33)
    compare(Typography.scaleFromPercent(75), 0.75)
    compare(Typography.scaleFromPercent(200), 2.0)
  }

  function test_a_typed_percentage_is_taken_as_a_whole_number() {
    compare(Typography.scaleFromPercent(110.4), 1.1)
    compare(Typography.scaleFromPercent(110.6), 1.1)
    compare(Typography.scaleFromPercent(199.9), 1.99)
  }

  function test_a_typed_percentage_outside_the_range_settles_on_the_nearest_end() {
    compare(Typography.scaleFromPercent(74), 0.75)
    compare(Typography.scaleFromPercent(201), 2.0)
    compare(Typography.scaleFromPercent(1000), 2.0)
  }

  function test_an_unusable_percentage_falls_back_to_unscaled() {
    compare(Typography.scaleFromPercent(0), 1.0)
    compare(Typography.scaleFromPercent(-5), 1.0)
    compare(Typography.scaleFromPercent("big"), 1.0)
    compare(Typography.scaleFromPercent(undefined), 1.0)
    compare(Typography.scaleFromPercent(null), 1.0)
  }

  function test_the_field_shows_back_the_percentage_it_stored() {
    compare(Typography.percentFromScale(1.1), 110)
    compare(Typography.percentFromScale(0.75), 75)
    compare(Typography.percentFromScale(2.0), 200)
    compare(Typography.percentFromScale(Typography.scaleFromPercent(110)), 110)
    compare(Typography.percentFromScale(Typography.scaleFromPercent(133)), 133)
    compare(Typography.percentFromScale(5), 200)
    compare(Typography.percentFromScale(undefined), 100)
  }

  function test_offered_bounds_match_the_supported_scale() {
    compare(Typography.MINIMUM_PERCENT, 75)
    compare(Typography.MAXIMUM_PERCENT, 200)
    compare(Typography.PERCENT_STEP, 5)
    compare(Typography.scaleFromPercent(Typography.MINIMUM_PERCENT), Typography.MINIMUM_SCALE)
    compare(Typography.scaleFromPercent(Typography.MAXIMUM_PERCENT), Typography.MAXIMUM_SCALE)
  }

  function test_a_typed_percentage_reaches_the_renderer() {
    compare(Typography.px(10, Typography.scaleFromPercent(110)), 11)
    compare(Typography.px(12, Typography.scaleFromPercent(110)), 13)
    compare(Typography.px(14, Typography.scaleFromPercent(110)), 15)
    compare(Typography.px(12, Typography.scaleFromPercent(1000)), 24)
  }
}
