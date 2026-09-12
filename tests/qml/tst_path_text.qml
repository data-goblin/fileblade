import QtQuick
import QtTest
import "../../lib/PathText.js" as PathText

TestCase {
  name: "PathIdentity"

  function test_dropped_urls_survive_spaces_in_either_form() {
    compare(PathText.droppedPath("file:///home/user/folder%20test"), "/home/user/folder test")
    compare(PathText.droppedPath("file:///home/user/folder test"), "/home/user/folder test")
    compare(PathText.droppedPath("file://localhost/home/user/folder%20test"), "/home/user/folder test")
    compare(PathText.droppedPath("file:///home/user/a%23b/c%20d.png"), "/home/user/a#b/c d.png")
  }

  function test_dropped_urls_reject_what_is_not_a_local_file() {
    compare(PathText.droppedPath(""), "")
    compare(PathText.droppedPath(undefined), "")
    compare(PathText.droppedPath("https://example.com/thing.png"), "")
    compare(PathText.droppedPath("file://server/share/thing.png"), "")
    compare(PathText.droppedPath("/home/user/plain"), "")
  }

  function test_blank_values_become_empty() {
    compare(PathText.pathText(undefined), "")
    compare(PathText.pathText(null), "")
    compare(PathText.pathText(""), "")
    compare(PathText.pathText("   "), "")
    compare(PathText.pathText("\t\n"), "")
  }

  function test_trailing_space_in_a_name_survives() {
    compare(PathText.pathText("/home/user/thing/ "), "/home/user/thing/ ")
    compare(PathText.pathText("/home/user/  two  "), "/home/user/  two  ")
    compare(PathText.pathText(" /home/user/lead"), " /home/user/lead")
  }

  function test_control_characters_in_a_name_survive() {
    compare(PathText.pathText("/home/user/thing\n"), "/home/user/thing\n")
    compare(PathText.pathText("\r\n/home/user/thing\t"), "\r\n/home/user/thing\t")
    compare(PathText.pathText("/home/user/thing \n"), "/home/user/thing \n")
  }

  function test_interior_characters_are_never_touched() {
    compare(PathText.pathText("/home/user/a b/c  d"), "/home/user/a b/c  d")
  }

  function test_byte_uri_does_not_alias_replacement_character() {
    var raw = "file:///fixture/%FF.txt"
    var unicode = "/fixture/�.txt"
    compare(PathText.normalize(raw, "/home/test"), raw)
    compare(PathText.normalize(unicode, "/home/test"), unicode)
    verify(PathText.fileUrl(raw) !== PathText.fileUrl(unicode))
    compare(PathText.name(raw), "\\xFF.txt")
    compare(PathText.name(unicode), "�.txt")
    compare(PathText.name("/fixture/\\xFF.txt"), "\\\\xFF.txt")
    compare(PathText.name("file:///fixture/%C3%A9%FF%F0%9F%90%B1.txt"), "é\\xFF🐱.txt")
  }

  function test_parent_join_and_subtree_remap_preserve_bytes() {
    var child = "file:///fixture/%FF/nested/file.txt"
    compare(PathText.parent("file:///fixture/%FF"), "/fixture")
    compare(PathText.parent(child), "file:///fixture/%FF/nested")
    verify(PathText.within(child, "/fixture"))
    verify(!PathText.within(child, "/fixture/�"))
    compare(PathText.relative(child, "/fixture"), "\\xFF/nested/file.txt")
    compare(PathText.join("file:///fixture/%FF", "100% #?.txt"), "file:///fixture/%FF/100%25%20%23%3F.txt")
    compare(PathText.remap(child, "/fixture", "/moved"), "file:///moved/%FF/nested/file.txt")
    compare(PathText.remap(child, "file:///fixture/%FF", "/plain"), "/plain/nested/file.txt")
    compare(PathText.remap("/plain/é.txt", "/plain", "file:///fixture/%FF"), "file:///fixture/%FF/%C3%A9.txt")
    compare(PathText.remap("/plain", "/plain", "/"), "/")
  }

  function test_normal_paths_and_literal_percent_sequences_keep_their_identity() {
    var names = ["plain.txt", "100%.txt", "%FF.txt", " leading ", "é.txt", "line\nbreak", "a+b[1]#?.txt", "\\xFF.txt"]
    for (var i = 0; i < names.length; i++) {
      var path = "/fixture/" + names[i]
      compare(PathText.fromFileUrl(PathText.fileUrl(path)), path)
      compare(PathText.join("/fixture", names[i]), path)
    }
    compare(PathText.normalize("../other", "/fixture/inside"), "/fixture/other")
    compare(PathText.fromFileUrl("file://localhost/fixture/%41%2f../b"), "/fixture/b")
  }

  function test_invalid_uris_are_not_reinterpreted_as_local_paths() {
    var values = ["file://remote/fixture/x", "file:///fixture/%", "file:///fixture/%ZZ", "file:///fixture/%00", "file:///fixture/x#y", "file:///fixture/x?y", "file:relative", "file:///fixture/a\x7fb", "file:///fixture/a b", "file:///fixture/a\\b"]
    for (var i = 0; i < values.length; i++) {
      compare(PathText.fileUrl(values[i]), "")
      compare(PathText.normalize(values[i], "/home/test"), values[i])
      verify(!PathText.within(values[i], "/"))
    }
  }
}
