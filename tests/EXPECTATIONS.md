# FileBlade UI expectations

> [!NOTE]
> The expectations are not complete. Please update them if you
> will modify this source code. - Kurt

The rest of this file is written by an agent.

---

Every entry describes something a person does in FileBlade and the result they
can see. Implementation details belong in the scripts, not in the expectation.
The expectations run in order, basic first, so an early failure explains later
failures.

This catalog covers the FileBlade host and its bundled Files, Properties, and
Notes modules. Optional companion plugins keep their own expectations.

Each expectation carries the script that proves it and the case label that
script prints. Set `OVM` to your headless VM harness executable, then run a section:

```
tests/vm/expectations/03-tree-keyboard.sh
```

Run everything, including guest setup:

```
tests/vm/expectations/run
```

Run a single case:

```
ONLY=E-11-07 tests/vm/expectations/11-actions-menu.sh
```

Ids never get reused. When behaviour changes, edit the entry in place.

## 1. Blades open, close and take focus

`tests/vm/expectations/01-blade-lifecycle.sh`

1. **E-01-01** If I press `Super+B` while the left blade is hidden, the left
   blade opens and is ready for keyboard input.
2. **E-01-02** When I open the left blade, I can see the file tree and file
   properties in the same blade.
3. **E-01-03** If I press `Super+B` while the left blade is open and focused,
   the blade hides completely.
4. **E-01-04** If I have the left blade open but not focused, pressing `Super+B`
   hides it: one press closes an open blade wherever my focus is, and one
   press opens and focuses a hidden one.
5. **E-01-05** If I press `Super+Shift+B`, the right blade opens and takes focus
   without hiding the left blade.
6. **E-01-06** After I focus a blade, my keyboard input controls the blade and
   does not type into the application behind it.
7. **E-01-07** While I use the keyboard in a blade, the last application window
   keeps its active appearance without receiving my typing.
8. **E-01-08** If I click an application window, keyboard focus leaves the
   blade and returns to that window.
9. **E-01-09** If I close the last application window while a blade is open,
   keyboard focus moves to the left blade when available, otherwise the right.
   Where my pointer rests decides it: over bare desktop the hover watch gives
   focus back to no one.
10. **E-01-10** If I move focus right from the left blade, it enters the right
    blade; moving focus left returns it to the left blade.
11. **E-01-11** If I click the Up button while the blade has keyboard focus, it
    works on the first click without needing a preliminary click.
12. **E-01-12** If I press `Super+B` while the left blade is focused and an
    application window is open, the blade closes and that window gets my
    keyboard focus again.
13. **E-01-13** If I press `Escape` in the properties pane, in a note, or in an
    extension's tree, the blade closes, the same as it does from the file tree.
    `Shift+Tab` is the key that moves to the previous section instead.

## 2. The tree draws what is on disk

`tests/vm/expectations/02-tree-rendering.sh`

14. **E-02-01** When I open a directory, I see each visible item in it exactly
    once.
15. **E-02-02** When I view a directory, folders appear before files and each
    group follows my chosen sort order.
16. **E-02-03** When I view folders, each one shows an expand control, including
    an empty folder; FileBlade does not add an “empty” label or badge.
17. **E-02-04** When I view a symbolic link, it has a distinct link icon and is
    sorted as a link rather than as its target.
18. **E-02-05** If another application creates a file in the directory I am
    viewing, the new file appears without me refreshing.
19. **E-02-06** If another application deletes a file in the directory I am
    viewing, the file disappears without me refreshing.
20. **E-02-07** If a collapsed folder changes and I refresh, expanding that
    folder shows its current contents.

## 3. Moving through the tree with the keyboard

`tests/vm/expectations/03-tree-keyboard.sh`

21. **E-03-01** If I press `j` or `Down`, the keyboard selection moves to the
    next row; `k` or `Up` moves it to the previous row.
22. **E-03-02** If I press `g` or `Home`, the keyboard selection moves to the
    first row; `G` or `End` moves it to the last row.
23. **E-03-03** If I press `Ctrl+D`, the list moves one page down; `Ctrl+U`
    moves it one page up. `Ctrl+B` also pages up without opening a prefix or
    changing the search layout.
24. **E-03-04** If I try to move beyond the first or last row, the keyboard
    selection stays at that end of the list.
25. **E-03-05** If I press `q`, the blade closes.
26. **E-03-06** If I press `Escape` when no dialog, menu, or helper is open, the
    blade closes.
27. **E-03-07** If I press `?`, the shortcut guide opens; pressing `Escape`
    closes the guide while leaving the blade open.

## 4. Expanding and collapsing

`tests/vm/expectations/04-expand-collapse.sh`

28. **E-04-01** If I select a folder and press `zo`, the folder expands in
    place. `zO` or `Shift+Right` recursively expands that folder and its child
    folders, leaving siblings and the tree root unchanged. `zR` expands the
    whole tree regardless of the selected row.
29. **E-04-02** If I select an expanded folder and press `zc`, the folder
    collapses. `zC` or `Shift+Left` also forgets its descendants' expansion,
    without collapsing siblings; reopening it shows only its direct children.
    `zM` collapses the whole tree. Fold commands never launch files, and Escape
    cancels an unfinished `z` sequence without closing the blade.
30. **E-04-03** If I expand an empty folder, it opens without showing children
    or an error.
31. **E-04-04** If I press `Enter` on a folder, it expands in the current tree
    instead of replacing the directory I am viewing.
32. **E-04-05** If I refresh the tree, folders I expanded remain expanded.
33. **E-04-06** If I expand a folder after it changed while collapsed, I see its
    current contents.

## 5. Selecting

`tests/vm/expectations/05-selection.sh`

34. **E-05-01** If I click a row, that row becomes the only selected item.
35. **E-05-02** If I press `v`, visual selection starts; pressing `j` extends
    the selection to the next row.
36. **E-05-03** If I press `Escape` during visual selection, visual mode ends
    while the blade remains open and an item remains selected.
37. **E-05-04** If I press `Ctrl+A`, every row in the directory becomes
    selected.
38. **E-05-05** If I press `Ctrl+Space`, the current keyboard row toggles in or
    out of the selection.
39. **E-05-06** When I select a different item, the properties pane updates to
    show that item.
40. **E-05-07** If I open the actions menu with several items selected, it shows
    how many items are targeted and disables actions that require one item.

## 6. Going somewhere else

`tests/vm/expectations/06-navigation.sh`

41. **E-06-01** If I press `Alt+Up`, I move to the parent folder; when there is
    no parent to visit, the Up action is unavailable.
42. **E-06-02** If I press `Backspace` or `Alt+Left`, I return to the previous
    location; `Alt+Right` takes me forward again.
43. **E-06-03** If I press `Alt+Home`, I move to my home directory.
44. **E-06-04** If I click the Up, Back, Forward, or Home buttons, they navigate
    to the same places as their keyboard shortcuts.
45. **E-06-05** If I enter a path that does not exist, I see an error and remain
    in the current directory.
46. **E-06-06** If I enter a valid path whose name ends with spaces, FileBlade
    opens that exact path rather than its parent.
47. **E-06-07** If I press `Backspace` while typing in the search or location
    field, it deletes a character and does not take me back a folder.
48. **E-06-08** If I open the Drives location (its icon sits next to Trash, or I
    enter `drives:///`), I see my volumes grouped as External, Not mounted,
    Internal, and System, each with a drive glyph and a mount or eject corner;
    Up is unavailable there, and a refused mount shows the reason under the
    list.

## 7. Hidden entries and Git presentation

`tests/vm/expectations/07-hidden-and-git.sh`

49. **E-07-01** When I show hidden files, each hidden file carries a
    crossed-out-eye mark on the corner of its own icon, so the marker slot at the
    left edge of the row stays free for the favourite star.
50. **E-07-02** If I press `.`, `Shift+H`, or `Ctrl+H`, hidden files toggle
    between shown and hidden.
51. **E-07-03** When I view a hidden file, its row is dimmed but I can still see
    its real Git status, and a changed hidden entry keeps its status colour at
    reduced strength rather than turning grey. With hidden entries turned off, a
    hidden entry that Git reports as changed is still listed, so every change the
    repository summary counts can be found in the tree; unchanged hidden entries
    stay out of it.
52. **E-07-04** When I view a Git-ignored file, it keeps its normal file icon
    and shows a do-not-enter sign in the Git status column.
53. **E-07-05** A Git repository folder has a repository icon, and I can see
    which branch it is on. When I open that repository, its root row replaces
    the chosen columns with a summary using Git's markers: M modified,
    A added, ? untracked, D deleted, R renamed, C copied, T type changed, and U conflicted.
    Markers precede counts everywhere. Zero change counts are omitted; an
    unchanged repository says Clean. Ahead/behind arrows appear only when an upstream comparison is
    available, using the last fetched state. Counts are bold and color-coded,
    with branch and worktree on their left, right-aligned at the first detail
    column's edge (or the name column's edge if there are no detail columns).
    The branch is dimmed. Only linked worktrees show a worktree name, separated
    from the branch by a space.
    In Git settings I can toggle branch, worktree, each count and the Clean
    indicator independently; my choices survive a restart. Turning every item
    off restores the chosen columns. Hovering explains the counts in matching
    colors, omitting zero counts and the word “files”.
    Repository folders merely listed in the tree keep the chosen columns.
54. **E-07-06** If I turn off Git status in settings, the Git status column,
    Git colors, and Git icons disappear; turning it on restores them.
55. **E-07-07** If I change the priority columns, the information shown on the
    right side of each row changes to match my choices, except the opened
    repository's root row, which keeps its Git summary.
56. **E-07-08** If a file inside a Git repository changes on disk, its Git status
    in the tree updates on its own without me refreshing.

## 8. Properties and previews

`tests/vm/expectations/08-properties-preview.sh`

57. **E-08-01** When I select a file, I can see its path, type, size, dates,
    permissions, and owner in the properties pane. A file with a preview card
    shows the card first, so I scroll to reach the rest.
    Each Git status appears on its own line; the next row of properties moves
    down together. Worktree appears only for a linked worktree, and Branch is dimmed.
58. **E-08-02** When I select a JPEG, PNG, or WebP image smaller than 16 MiB, I
    see an inline preview without another click.
59. **E-08-03** When I select an image larger than 16 MiB, I see a clear message
    explaining that it is too large to preview.
60. **E-08-04** When I select a symbolic link to an image, I see a clear message
    explaining why FileBlade will not preview it.
61. **E-08-05** When I select a symbolic link to a text file, I see the same
    clear link explanation rather than a technical error code.
62. **E-08-06** When I select a broken symbolic link, I can see its intended
    target and a correct explanation that the target is missing.
63. **E-08-07** When I view a text preview with more content than fits, its card
    keeps a stable size and shows a chevron for revealing more.
64. **E-08-08** When I select a folder, I see its folder details and no file
    preview.
65. **E-08-09** When nothing is selected, the properties pane shows the
    FileBlade wordmark dimmed to the muted text tone instead of an empty box.
66. **E-08-10** When I select an image the shell does not decode, such as an SVG
    or a GIF, no preview box appears at all; the properties show without an empty
    card, and opening the file still uses the desktop default application.

## 9. Searching

**E-32-01** (`tests/vm/expectations/32-search-visibility.sh`):
The Auto-hide search bar setting is off by default. When enabled, Files and
extension search bars are hidden until the configured search shortcut (`/` by
default) reveals them. Leaving the field hides it again while preserving the
filter; Escape clears the filter and returns to the tree. Disabling the setting
keeps the bars visible. The choice survives a shell restart.

`tests/vm/expectations/09-search.sh`

66b. **E-09-00** The Files search field sits below the project header, the
    favorites and the volumes, directly above the navigation icons, so the
    project and favorites stay at the top of the blade.
67. **E-09-01** If I press `/`, the search field takes focus so I can type
    immediately.
68. **E-09-02** When I type a plain search, the current tree is filtered and I
    can see how many items match out of the total.
    Exact filenames and prefixes appear before matches with gaps between
    letters, regardless of the selected column sort or folders-first order.
    Matching branches stay together, with their parents kept for context;
    equally relevant matches keep my chosen sort order. Extension trees use
    the same matching priorities within and between their groups. Each group
    stays together and appears once, even with nested groups.
    Regular search uses the tree's single-line rows and padding, without
    repeating the filename underneath. Extension tree rows have the same
    height and indentation step. Deep search only adds a path line when it
    provides information beyond the filename.
69. **E-09-03** When I clear the search, the full directory listing returns.
70. **E-09-04** If I press `Ctrl+F`, FileBlade searches the entire current
    folder and shows how many results it found.
71. **E-09-05** If I turn case sensitivity on or off, the matching results
    change accordingly.
72. **E-09-06** If I enable regular expressions and enter a valid pattern, the
    results match that pattern.
73. **E-09-07** If I enter an invalid regular expression, I see an explanation
    of the error rather than a misleading zero-results message.
74. **E-09-08** If I press `Up` in the search field, I can revisit my previous
    searches.
75. **E-09-09** If I click the search field and then move my pointer around
    inside the same Files section, the search field keeps keyboard focus and
    my typing still lands in it.
76. **E-09-10** Search focus is not sticky: if I move my pointer to another
    Hyprland window, to the other blade, or to another section of the same
    blade, keyboard focus leaves the search field and follows the pointer as
    it does everywhere else.
77. **E-09-11** Search focus is not sticky for the keyboard either: if I move
    focus to another blade or another section with a key binding, keyboard
    focus leaves the search field.
78. **E-09-12** When the search field is empty, the `Aa`, `.*`, and `fzf` chips
    sit flush with its right edge; they step left only while the `×` clear
    button is showing.

## 10. Quick navigation

`tests/vm/expectations/10-quicknav.sh`

79. **E-10-01** If I press `Shift+Z`, quick navigation opens with my most
    relevant and frequently visited folders near the top. The empty search bar
    reminds me of that key in parentheses after "Search...", and it shows my
    own key if I rebound quick navigation.
80. **E-10-02** When I type in quick navigation, the folder choices narrow to
    match my text.
81. **E-10-03** If I select a folder and press `Enter`, FileBlade opens that
    folder.
82. **E-10-04** If I press `Escape`, quick navigation closes and I remain in my
    current folder.

This file was written by an agent.

- **E-10-05** The Screenshots shortcut opens my configured pictures directory,
  including a custom path in `$XDG_CONFIG_HOME/user-dirs.dirs`. Explicit screenshot
  and pictures environment settings keep their precedence.
- **E-10-06** With View hidden off, Quick Nav, the file tree, search and Recent
  omit cache directories carrying a valid `CACHEDIR.TAG`, their contents, and
  FileBlade's private configuration, state and thumbnail cache. Remembered
  visits do not bring them back. View hidden reveals them; ordinary projects,
  including FileBlade's source checkout, remain visible either way.
- **E-10-07** Changing the Quick Nav query selects the first new result, even
  when existing rows move. Arrow keys still select another result, and a
  refresh of the same query preserves that choice. My current folder stays
  excluded from Quick Nav.

## 11. The actions menu

`tests/vm/expectations/11-actions-menu.sh`

83. **E-11-01** If I right-click a row, its actions menu opens beside it and the
    clicked row becomes the target.
84. **E-11-02** If I select a row and press `m`, the same actions menu opens for
    that row.
85. **E-11-03** When the actions menu opens, I can type immediately to filter
    its actions.
86. **E-11-04** If I press `Enter` after filtering the actions menu, the first
    available matching action runs.
87. **E-11-05** If I press `Down` or `Up` in the actions menu, focus moves among
    available actions and skips unavailable ones.
88. **E-11-06** In the actions menu, I see the same keyboard shortcut for each
    action that I see in the `?` shortcut guide.
89. **E-11-07** If I continue past the last action with `Down`, focus reaches
    the color swatches; `Left` and `Right` choose a color and `Enter` applies it.
90. **E-11-08** If I apply a folder color with the keyboard, that color remains
    after the shell restarts.
91. **E-11-09** If I press `Escape`, the actions menu closes while the blade
    remains open and ready for keyboard input.
92. **E-11-10** If I choose Rename from the menu or press `r`, the rename field
    contains the current name with its text selected for replacement.
93. **E-11-11** If I choose New file or New folder from the menu or shortcut,
    the name field starts empty and Create remains unavailable until I type a
    valid name.

## 12. Creating and renaming

`tests/vm/expectations/12-create-rename.sh`

94. **E-12-01** If I press `a` or `Ctrl+N`, enter a valid name, and confirm, a
    file with that name appears in the current folder.
95. **E-12-02** If I press `Ctrl+Shift+N`, enter a valid name, and confirm, a
    folder with that name appears in the current folder.
96. **E-12-03** If I select an item and press `r` or `F2`, I can rename it; after
    I confirm, only the new name appears.
97. **E-12-04** If I try to rename an item to a name that already exists, I see
    a refusal and neither item changes.
98. **E-12-05** If I enter a name containing `/`, I cannot confirm it and no
    item is created or renamed.
99. **E-12-06** If I enter `.`, `..`, or `../escape` as a name, FileBlade
    refuses it and nothing is created or renamed.
100. **E-12-07** If I enter only whitespace as a name, I cannot confirm it.
101. **E-12-08** If I enter an otherwise valid name ending in a space, FileBlade
     creates the item with that exact trailing space.

## 13. Copying, cutting, pasting and dragging

`tests/vm/expectations/13-transfer.sh`

102. **E-13-01** If I select items and press `y` or `Ctrl+C`, they are ready to be
     copied elsewhere.
103. **E-13-02** If I select a destination folder and press `p` or `Ctrl+V`, the
     copied items appear in that folder.
104. **E-13-03** If I press `x` or `Ctrl+X` and then paste into another folder,
     the items move there and disappear from their original location.
105. **E-13-04** If I paste an item where the same name already exists, FileBlade
     creates a separately named copy instead of overwriting either item.
106. **E-13-05** If I drag an item onto a folder, the item moves into that
     folder.
107. **E-13-06** If I pause over a collapsed folder while dragging, the folder
     expands so I can choose a destination inside it.
108. **E-13-07** If I try to drop an item onto itself or into one of its own
     descendants, FileBlade refuses the drop and leaves everything unchanged.
109. **E-13-08** If I choose Copy path, I can paste the selected item's exact
     path into another application.
110. **E-13-09** When I hover, single-click, or double-click an item, the mouse
     cursor is a pointing finger.
111. **E-13-10** While I click-drag any item, the mouse cursor is a grab hand for
     the entire drag, even when I move it outside the blade.
112. **E-13-11** I see the same cursor behavior and appearance in the left and
     right blades unless a FileBlade plugin deliberately supplies its own
     cursor for its interface.

## 14. Trash

`tests/vm/expectations/14-trash.sh`

Dialog keyboard regressions: `tests/vm/trash-dialog-focus.sh`.

113. **E-14-01** If I select items and press `d` or `Delete`, FileBlade asks me
     to confirm before moving them to Trash. With blades on several monitors
     the confirmation appears on each of them, answering it on any one monitor
     closes it on all the others, and confirming once moves the items once. A
     confirmation left open on a hidden blade never answers a newer request:
     the newer request replaces it, and a stale prompt only closes.
114. **E-14-02** If I cancel the trash confirmation, the selected items remain
     in their original locations.
115. **E-14-03** If I confirm the trash action, the items disappear from their
     original locations and become available in the Trash view.
116. **E-14-04** When I open the Trash view, each item shows its original name
     and the date it was trashed.
117. **E-14-05** When I hover over a trashed item, Restore, Reveal, and Delete
     appear while the item's other details remain readable.
118. **E-14-06** If I choose Restore, the trashed item returns to its original
     location.
119. **E-14-07** If I choose Delete permanently, FileBlade asks first; `Enter`
     confirms and `Escape` cancels.
120. **E-14-08** If I press `Tab` in the permanent-delete confirmation, focus
     stays inside the dialog and does not activate anything behind it.
121. **E-14-09** If I confirm permanent deletion, the item disappears from the
     Trash view and cannot be restored from it.
122. **E-14-10** If I choose Empty Trash, FileBlade asks first; after I confirm,
     the Trash view is empty.
123. **E-14-11** Items I trash in FileBlade also appear in the system Trash with
     their original locations, so other desktop tools can restore them.
124. **E-14-12** In the Trash view, `j` and `k` move the selection down and up, and
     `g` or `Shift+G` jump to the first or last item.

## 15. Undo and redo

`tests/vm/expectations/15-undo-redo.sh`

125. **E-15-01** If I press `u` or `Ctrl+Z`, the most recent operation is undone
     and FileBlade names the operation I can undo.
126. **E-15-02** If I undo a trash action, the item returns to its original
     location.
127. **E-15-03** If I press `Ctrl+Shift+Z` after undoing something, FileBlade
     performs that operation again.
128. **E-15-04** If I undo a rename, the item's previous name returns.
129. **E-15-05** If I undo a move, the item returns to its source folder.
130. **E-15-06** If I press the undo shortcut when there is nothing to undo, no
     undo operation is shown and nothing changes.
131. **E-15-07** If I undo a color change, the item returns to the color it had
     before and FileBlade names the color operation.

## 16. Favorites

`tests/vm/expectations/16-favorites.sh`

132. **E-16-01** If I pin a folder, it appears in Favorites and its row shows
     that it is pinned.
133. **E-16-02** If I unpin a favorite, it disappears from Favorites and loses
     its pinned marker.
134. **E-16-03** After the shell restarts, my favorite folders are still listed.
135. **E-16-04** If a favorite no longer exists, I can still open Favorites and
     browse the rest of my files normally.

## 17. Settings

`tests/vm/expectations/17-settings.sh`

136. **E-17-01** If I click the gear, FileBlade's settings open.
137. **E-17-02** If I resize a blade, it reopens at the width I chose.
138. **E-17-03** If I change Show hidden files in settings, I get the same
     visible or hidden result as using the keyboard shortcut.
139. **E-17-04** If I change where Properties appears, the properties panel
     visibly moves to the chosen place.
140. **E-17-05** If I change Trash retention or trash confirmation, my choices
     remain after I close and reopen FileBlade.
141. **E-17-06** If I choose Shortcuts in settings, I see the same shortcut
     guide that opens when I press `?`.
142. **E-17-07** If settings are open and I press `Escape`, settings close first
     and the blade remains open.
143. **E-17-08** If I press `,` while a blade has keyboard focus, that blade's
     settings sheet opens; `Escape` closes it and the blade stays open.
144. **E-17-09** In the settings sheet I see every tab of every section as its
     own row with a grip, not a collapsed "+2" count, and I can drag a row to
     reorder tabs or move one to another section.
145. **E-17-10** If I hover an icon button such as the gear or refresh, its tip
     shows the action after a mouse glyph and the shortcut after a keyboard
     glyph, in the same layout everywhere; the gear reads Open settings and `,`.
146. **E-17-12** The Files settings are grouped under Tree, Git, and Trash and
     drives headings instead of one flat list, and filtering by a heading name
     shows the rows under it.
147. **E-17-11** If I set Mode badge to Footer in the Files settings, the Neovim
     mode badge leaves the header and sits at the bottom-left of the blade, left
     of the folder name; Hidden removes it, Header puts it back, and the choice
     survives a restart.
147b. **E-17-13** If I type a Font size percentage in the General settings, the
     text in every blade grows or shrinks by that amount while the blade widths
     stay where I put them, a value outside the range settles on the nearest
     end, and the choice survives a restart.
147c. **E-17-14** Ctrl+= (or Ctrl++) makes the text in every blade one step
     larger, Ctrl+- one step smaller, and Ctrl+0 returns it to 100%, wherever
     my focus is inside FileBlade: the tree, a search field, Notes, a picker or
     the settings sheet. It is the same Font size as the General setting, so the
     percentage there follows and the choice survives a restart. Plain + and -
     keep changing the tree density and media tile size.

## 18. Persistence

`tests/vm/expectations/18-persistence.sh`

148. **E-18-01** When I create a file in FileBlade, it appears immediately in
     FileBlade and in other applications that view the folder.
149. **E-18-02** If I close and reopen a blade, it returns to the same folder and
     shows files I created there.
150. **E-18-03** After the shell restarts, FileBlade remembers my current
     folder, folder colors, favorites, and settings.
151. **E-18-04** After I update FileBlade, my saved locations, folder colors,
     favorites, and settings remain intact.

## 19. Refusals, errors and edges

`tests/vm/expectations/19-refusals.sh`

152. **E-19-01** If FileBlade refuses an action, I see an explanation instead of
     nothing happening.
153. **E-19-02** If an operation fails, its error clears after I have had time to
     read it and does not prevent my next action.
154. **E-19-03** If I try to open a folder I cannot read, I see a permission
     error rather than an empty folder.
155. **E-19-04** If a file disappears while FileBlade is acting on it, I see a
     message that the file is missing.
156. **E-19-05** If I enter a name longer than the filesystem allows, FileBlade
     refuses it and explains the problem.
157. **E-19-06** If loading or an operation takes too long, I see a timeout error
     rather than an endless spinner.

## 20. Arranging blade sections and tabs

Automation for this section is pending.

158. **E-20-01** If I click the caret beside a section title such as Properties,
     that section collapses to its title bar and leaves the rest of the blade
     available; clicking the caret again expands it.
159. **E-20-02** If I close and reopen a blade, each section keeps the expanded
     or collapsed state I chose.
160. **E-20-03** If I drag a section title above or below another section in the
     same blade and release it, the section drops into that vertical position.
161. **E-20-04** If I drag a section title to the other blade and release it, the
     section moves to that blade.
162. **E-20-05** If I drag a module onto another section and release it, it joins
     that section as a tab; I can also drag its tab to a different position in
     the tab row.
163. **E-20-06** While I drag a section between vertical positions, I see a
     horizontal insertion line showing exactly where it will land.
164. **E-20-07** While I drag a module onto a section body, I see that section
     outlined as the target for the new tab.
165. **E-20-08** While I drag a module within a tab row, I see only a slim `|`
     between tab titles at the exact insertion point; the existing tab titles
     remain clear and unobscured.
166. **E-20-09** If I release a dragged section or tab over a valid target, it
     stays in the indicated position; if I release it over no target, its layout
     does not change.
167. **E-20-10** If I close and reopen a blade after arranging its sections and
     tabs, the layout I chose is preserved.
168. **E-20-11** Layout changes do not take over `Ctrl+Z`; it remains the undo
     shortcut for file operations. I can reverse a layout move by dragging the
     section or tab back, or by using the layout controls in Settings.
169. **E-20-12** If I drag the divider between two expanded sections, they resize
     together and reopen at the sizes I chose.

## 21. Docking, resizing and window behavior

`tests/vm/expectations/21-docking-resizing.sh`

170. **E-21-01** When I open a docked blade, my tiled windows move aside so the
     blade does not cover them.
     Maximized windows and videos expanded within their browser window also
     stay clear of both blades. True fullscreen (`Super+F`) and browser video
     fullscreen hide the blades until I leave fullscreen, then restore them
     with the same widths and sections. Hidden blades do not keep keyboard
     focus. See `33-fullscreen.sh` for these cases.
171. **E-21-02** If I press `Super+T` while a blade is focused, it becomes a
     regular window that I can tile and move; pressing `Super+T` again docks it.
172. **E-21-03** If I press `Super+-` or `Super+=` while a blade is focused, only
     that blade becomes narrower or wider.
173. **E-21-04** If I drag a docked blade's inner edge, the blade follows the
     pointer without smearing or stretching its contents.
174. **E-21-05** If I close and reopen a blade after resizing or docking it, it
     keeps the width and docked or window mode I chose.
175. **E-21-06** If I press `Super+W` while a blade is focused, that blade closes;
     if no blade is focused, my active application window closes instead.
176. **E-21-07** If I use `Super` with an arrow key, focus moves naturally among
     my application windows and open blades in that direction.
177. **E-21-08** If I use `Super+Shift` with an arrow key while a blade section is
     focused, that section moves in the chosen direction; outside a blade, my
     normal window-swap behavior remains unchanged.
178. **E-21-09** If I turn blade animations off, blades appear and disappear
     without sliding; turning animations on restores the slide.
179. **E-21-10** On a multi-monitor setup, blades appear only where the
     Monitors setting allows and interact with windows on the same screen. The
     Monitors dropdown in Settings offers Active (the default), All, and one
     "Lock to" entry per detected monitor. With Active, a blade opens on the
     monitor I am working on and stays there; it does not follow my focus. If
     I press its shortcut while working on another monitor, the open blade
     closes, and the next press opens it on the monitor I am on. With a lock,
     blades only ever appear on that monitor and the shortcuts act there
     wherever I am. My layout, notes and selection are the same on every
     monitor. See section 34 for the two-monitor checks.
179b. **E-21-10b** If a monitor is unplugged, a blade that was invoked or
     locked there stays hidden until I close and reopen it or the monitor
     returns; a menu, drop wheel, drag or keyboard focus that lived on it is
     cancelled rather than moved; my lock setting is kept.
179c. **E-21-11** If I hold `Super` and drag with the right mouse button over a
     docked blade, the blade resizes under the pointer the way `Super` and the
     right button resize any other window, and the width I release at is the
     width it keeps.
179d. **E-21-12** If I hold `Super` and drag with the right mouse button anywhere
     that is not a docked blade, the gesture resizes the window underneath
     exactly as it did before: Hyprland keeps its own binding and FileBlade
     listens beside it without consuming the press, so ordinary window resizing
     is untouched whether FileBlade is running or not.
179a. **E-21-10a** A blade width I set on a large monitor never exceeds what a
     smaller monitor can show: on that monitor the blade renders and reserves
     at most its own screen's limit, and my stored width is left alone.

## 22. Adding and managing module tabs

`tests/vm/expectations/22-module-picker.sh` covers the module picker.
Automation for the remaining tab-management scenarios is pending.

180. **E-22-01** If I click `+` in a section's tab row, I can search for a module
     and add it as another tab in that section. Both blades also show a module
     `+` beside the heading when a section has only one module, including Notes.
     Notes' separate note `+` still adds a note. The module picker has an
     "Add module" heading and close `×`; Escape closes only the picker, even
     after I hover its rows. Picking an extension adds it to the section whose
     `+` I clicked.
181. **E-22-02** If I add a module from Settings, it appears as a new section in
     the left or right blade I chose.
182. **E-22-03** If I click a tab, I see that module; `Ctrl+Tab`, `Ctrl+]`, and
     `Ctrl+PageDown` move to the next tab, while their reverse shortcuts move to
     the previous tab.
183. **E-22-04** If a section has more tabs than fit, I can scroll the tab row and
     the active tab is brought into view.
184. **E-22-05** If I rename a FileBlade tab, its custom name appears in the tab
     row and remains after reopening the blade; clearing the name restores its
     normal module title.
185. **E-22-06** If I close a tab with its `×`, FileBlade asks for confirmation;
     if the section's tabs change while that question is open, for example
     from another monitor, the question closes instead of removing a
     different tab;
     middle-clicking a tab closes it directly.
186. **E-22-07** I cannot close the only tab in a section by mistake; I remove the
     whole section from Settings when that is what I intend.
187. **E-22-08** In Settings, I can move a section up or down, send it to the
     other blade, or remove it, and I see the layout change immediately.
188. **E-22-09** If a module is disabled, incompatible, missing, or fails to load,
     I see a clear explanation and can choose another module instead of seeing
     a blank section.
189. **E-22-10** Each tab remembers its own content and choices when I switch
     tabs, close and reopen the blade, or restart the shell.
190. **E-22-11** If I right-click a tab, the menu offers to move it to the other
     blade with an arrow icon pointing that way: "Move to right blade" from the
     left blade, "Move to left blade" from the right. Choosing it takes the
     tab out of its section and places it as the top section of the other
     blade, opening that blade if it was closed, and focus follows the tab.
     When the blade has more than one section, the menu also offers "Move to
     top" or "Move to bottom" with an up or down arrow icon, which appends the
     tab to that section and makes it current there. "Close to the right"
     closes every tab after this one in the section after the same
     confirmation as closing a tab; it is disabled on the last tab. Every
     entry carries an icon.
191. **E-22-12** The close mark on a tab only appears while my pointer is over
     that tab; otherwise the tab shows its title alone.

## 23. Opening files, locations and recent items

`tests/vm/expectations/23-opening.sh`

192. **E-23-01** If I select a file and press `Enter` or `o`, it opens in its
     default application. On a folder, `Enter` toggles expansion in place and
     `o`, `l` or Right enters it as the tree root; `h` or Left goes to the
     parent directory.
193. **E-23-02** If I select a file and press `Shift+Enter`, I can choose from
     compatible applications and open the file with my choice.
194. **E-23-03** If I select one file and press `e`, it opens in LazyVim.
195. **E-23-04** If I press `Enter`, `Shift+Enter`, or `e` from Properties, the
     selected item opens the same way it does from the file tree.
196. **E-23-05** If I press `r` from Properties, my file manager opens with the
     selected item revealed.
197. **E-23-06** If I press `Ctrl+L`, I can type or paste a location and open it;
     an invalid location shows an error without moving me elsewhere.
198. **E-23-07** If I click Recent, I see files and folders I opened recently,
     ordered so useful recent choices are easy to reach.
199. **E-23-08** If I choose an item from Recent, it opens normally and remains
     available in my recent history.
200. **E-23-09** If I have no recent items, the Recent view tells me so instead of
     showing a blank or broken list.
201. **E-23-10** If I press `Ctrl+P`, I can use the picker to find files, recent
     items, content matches, or actions, and its visible prefixes tell me which
     mode I am using.
202. **E-23-11** If I double-click a file, it opens in its default application;
     double-clicking a folder opens it as the tree root, whether it is collapsed
     or expanded. `Enter` and the disclosure arrow still expand it in place.
203. **E-23-12** If I double-click or press `Enter` on a file listed by a companion
     module such as Memory, Skills, Hooks, or MCP, it opens in the same default
     application it would open in from the file tree, not in the terminal editor.

## 24. Search syntax, columns and colors

`tests/vm/expectations/24-search-columns-colors.sh`

204. **E-24-01** When I search, I can use quoted text for an exact phrase,
     `-word` or `!word` to exclude text, and `^start` or `end$` to anchor a match.
205. **E-24-02** When I search, I can narrow results with `type:`, `format:`, and
     `in:` filters.
206. **E-24-03** If I search with `content:`, I see matching lines grouped under
     their files; if I use `scope:everywhere`, I can find matching paths beyond
     the current folder.
207. **E-24-04** If I press `Ctrl+Shift+B` during a deep search, the results
     switch between a flat list and a tree without changing the search.
208. **E-24-05** If I click the add-column control, I can choose another detail
     to show beside each row; I can also remove a column I no longer want.
209. **E-24-06** If I drag a detail column, it moves to the indicated horizontal
     position without obscuring the other column labels.
210. **E-24-07** If I click a column label or use its menu, I can sort ascending,
     descending, add it as a secondary sort, or clear sorting; numbered arrows
     show the order of multiple sorts.
211. **E-24-08** If I filter a date or number column, only matching rows remain
     and a visible filter marker stays until I clear the filter.
212. **E-24-09** If I close and reopen a blade, my chosen columns, their order,
     sorting, and filters are preserved.
213. **E-24-10** If I choose which Git details to show, the Git status column
     displays those details without replacing the item's normal file icon.
     Counts use the same marker-first notation as the repository summary;
     the ? count includes only untracked files, never staged additions.
214. **E-24-11** If I color one or more files or folders, I can choose a palette
     color or enter a six-digit custom color and see it applied immediately.
215. **E-24-12** If I change the color scope, the color applies only to the icon,
     the name, or the whole row as selected; Reset returns to the theme color.
216. **E-24-13** If I open a column's menu, it has a heading (Columns, Add
     column, or Git status) with an `×` that closes it, above the filter field.
217. **E-24-14** The folder colour swatches never match a Git status colour;
     the red, yellow and green swatches are visibly rose, lemon and mint next to
     the amber, green and red Git markers. If I write my own hex colours into
     `colors.json` in FileBlade's config folder, the swatches and every coloured
     row take them without a restart; a bad value is ignored and the previous
     palette stays.

## 25. Notes

New notebooks begin with the supplied Lovecraft quotation. Existing notes,
including intentionally empty notes, retain their saved contents.

This file was written by an agent.

The last-edit time, word count and character count use the muted theme color
so they remain less prominent than the note text. Canonical Material Design
glyphs from Nerd Fonts replace the labels: clock-edit-outline for last edit,
text for words, and alphabetical for characters. Until a note has been edited,
the last-edit field is absent, with no placeholder or empty gap. Counts remain
visible, including zero; screen readers receive the full labels and values.

`tests/vm/expectations/25-notes.sh`

218. **E-25-01** If I add the Notes module, I can type a plain-text note directly
     in the blade.
219. **E-25-02** When I stop typing, switch modules, collapse Notes, or close the
     blade, my note is saved without a separate Save action.
220. **E-25-03** If I click `+` in Notes, a new named note tab appears and I can
     keep different text in each note. Default note numbers keep increasing
     after I close notes or restart, so deleting Note 1 does not create
     another Note 2.
221. **E-25-04** If I switch note tabs, each note keeps its own text and the tab I
     chose is visibly active.
222. **E-25-05** If I double-click a note tab or choose Rename from its menu, I
     can rename that note.
223. **E-25-06** If I middle-click a note tab or choose Close note, that note
     closes; FileBlade keeps at least one note so I cannot lose the notebook
     itself by closing the last tab.
224. **E-25-07** If I close and reopen the blade or restart the shell, my note
     names, text, and active note are preserved.
225. **E-25-08** If my notes reach their storage limit, FileBlade warns me and
     stops accepting excess text instead of silently losing saved content.

## 26. Dragging files outside a blade

This file was written by an agent.

`tests/vm/expectations/26-drop-wheel.sh`

226. **E-26-01** When I drag files outside a blade, a compact ghost follows the
     pointer with the last grabbed row's icon and name.
227. **E-26-02** If I drag several selected items, the ghost shows how many items
     I am carrying while still naming the last grabbed row.
228. **E-26-03** If I hold my configured drop-wheel modifier during the drag, a
     wheel opens at the pointer and shows actions for the items I am carrying.
     Holding Space before the pointer leaves the blade works too.
229. **E-26-04** When I drag over an application, editor, terminal, or empty
     desktop, the wheel offers actions that make sense for that target.
230. **E-26-05** I can move around the wheel with the pointer, scroll wheel,
     arrow keys, `h/j/k/l`, or `Tab`, and the highlighted choice is always clear.
231. **E-26-06** If an action offers placements such as a tab, split, pane, or
     window, I can enter that second ring and choose the exact destination.
     The Herdr and tmux opening actions offer horizontal and vertical splits without an automatic
     "New pane" choice. Their icons show a full-width bottom pane and a
     full-height right pane respectively.
232. **E-26-07** If I release the drag on a valid wheel choice, FileBlade runs the
     highlighted action once for all carried items.
     If I release while the wheel's rows are still loading, FileBlade remembers
     the release point for at most 800 ms and runs the action there once the rows
     arrive; past that the wheel stays open for an explicit choice.
233. **E-26-08** If I cancel the wheel or release without a valid choice, no file
     is opened, moved, or changed. Escape during a held drag cancels it whether
     the wheel is open or closed: the ghost disappears, the blade stays open,
     and releasing the mouse afterward cannot drop files or paste paths.
234. **E-26-09** If I release the drag on the hub of the wheel, the wheel stays
     open, so I can still pick with the pointer or a key instead of losing it.
235. **E-26-10** In the drop wheel, the Open with wedge shows an open-folder
     glyph whatever I am dragging, and its placements list the applications for
     those files.

**E-26-11** When I choose Review with hunk over a Herdr or tmux terminal,
I can choose Vertical split, Horizontal split, New tab, New space, or New
window, with the same icons as the herdr and tmux opening actions. In tmux, a tab is
a tmux window and a space is a session; New window opens a separate terminal.
Each destination reviews the selected paths from their repository. Hunk is
greyed out when those paths have no Git status changes, including files outside
Git. Committing or reverting the changes also prevents an already-open wheel
from launching an empty review. Over a plain terminal or empty desktop, Hunk opens directly
in a new terminal without offering multiplexer destinations.
Covered by `tests/vm/expectations/26-hunk-review.sh`.

**E-26-12** If the terminal I drop on shares one process with other windows
(ghostty, kitty in single-instance mode, foot in server mode, wezterm), the
wheel only offers pane, tab, space and paste placements when it can tell
which window I dropped on: for Herdr that is the window whose title names
exactly one workspace across my Herdr sessions, checked again when I pick a
placement. Otherwise the target reads "shared window", those placements are
missing, the same goes for "This nvim", and picking one anyway is refused
with the reason; Open in new terminal and Review with hunk in a new window still
work. tmux gives no way to tell such windows apart, so it always counts as
shared there.

**E-26-13** Open in new terminal opens the selected files in nvim. A folder-only
selection opens a shell in that folder. Herdr splits target the currently
focused pane and tab, even when other tabs contain nvim or an older shell.

## 27. Updates and recovery

`tests/vm/expectations/27-updates-recovery.sh`

236. **E-27-01** If FileBlade or an enabled companion has an update available, I
     see an Update available chip in the blade footer.
This file was written by an agent.

237. **E-27-02** The update notice names the available FileBlade version, for
     example "Version 0.1.2 of FileBlade is now available!", followed by a
     Companion updates heading and one bullet per extension, ordered by name.
     The version headline, bullets and instructions stay readable at the minimum
     280-pixel blade width, and both buttons remain visible. It shows no commit
     counts. If a version cannot
     be determined, it says so; changes within the same version or to an older
     version are described accurately. A release tag names the update only when
     it is the highest valid version and resolves to the checked branch tip;
     otherwise the checker needs that tip's manifest already stored locally.
     The notice says FileBlade only checks and does not install while running,
     tells me to stop the shell, run `omarchy plugin update`, then run
     `omarchy restart shell`, and keeps Close and Check again.
238. **E-27-03** If my FileBlade checkout has local work or commits that must
     not be overwritten, the update details tell me it was skipped.
239. **E-27-04** If FileBlade's interface and native helper are out of sync after
     an update, I see Backend update needed and instructions to update or reinstall it.
240. **E-27-05** If update checking is disabled or the network is unavailable,
     I can continue using FileBlade without repeated prompts or an endless busy
     indicator.
241. **E-27-06** If I check for updates and nothing is newer, I see "FileBlade is
     up to date!" in the footer and it disappears by itself after ten seconds.

## 28. Archives and long operations

`tests/vm/expectations/28-archives-operations.sh`

242. **E-28-01** If I select a supported archive, Extract here is available and
     places its contents in a clearly named destination beside the archive.
243. **E-28-02** If I extract an archive and its destination is already
     populated, FileBlade refuses, explains the conflict, and leaves the
     existing files unchanged.
244. **E-28-03** While a long file operation is running, I can see what FileBlade
     is doing and stop the operation from Properties.
245. **E-28-04** If I stop an operation, items already completed remain visible
     and any partial destination is refreshed immediately so I can inspect it.
246. **E-28-05** If an undo cannot continue safely, I see why; pressing `Shift+U`
     lets me skip that refused undo and continue to an older one.

## 29. Welcome, core blades and help

`tests/vm/expectations/29-welcome.sh`

247. **E-29-01** On first launch, Welcome opens beside the persistent Notes
     tab. It introduces Files, Notes, Skills, Memory, Hooks and MCP, with
     keyboard-accessible entries and real help available offline.
248. **E-29-02** Opening a core entry selects its existing blade or adds it
     once. It requires no companion installation, registry or shell restart.
     The old welcomeInstall IPC reports "built-in" and starts no installer.
249. **E-29-03** Close Welcome records dismissal and removes only Welcome.
     Notes and other tabs retain their contents. A read-only or unready
     layout refuses dismissal.
250. **E-29-04** Welcome can be reopened from the blade picker after dismissal
     or a legacy "installed" state. Reopening preserves that saved state;
     later state/layout hydration and restart do not prune the reopened tab.
251. **E-29-05** Core entries and local help remain useful offline. An absent,
     malformed, oversized or newer-schema optional catalog retains the local
     or last-valid catalog. Its identity, source, compatibility and lifecycle
     fields are metadata; catalog-supplied commands never execute.

This file was written by an agent.

**E-29-06** If I enable Memory, Skills, MCP, Hooks, or a generated extension
before installing FileBlade, one pop-up lists the waiting extensions and shows
the FileBlade repository address. There is no Install button, and Enter does
nothing. Escape or the close button dismisses the pop-up. If FileBlade is already
installed but disabled, Enable activates that local installation and restarts the
shell. Once FileBlade is enabled, the pop-up disappears.

## 30. Configurable tree keys

`tests/vm/expectations/30-keybindings.sh`

252. **E-30-01** Editing `keybindings.json` changes tree navigation without a
     shell restart; overridden keys stop performing their old action and omitted
     actions keep their defaults.
253. **E-30-02** An empty binding array disables an action; key sequences can
     use a custom prefix and cancel on Escape or focus loss without opening,
     copying or deleting an item.
254. **E-30-03** Conflicting bindings preserve the last valid map and show an
     error. An action this FileBlade does not know, or a binding it cannot
     read, is dropped and named in the error while every other binding in the
     file still applies, so a file shared with a newer FileBlade keeps working.
     Fixing or removing the file clears the error and restores the
     corresponding bindings. A `keybindings.json` written by a newer FileBlade
     is never rewritten by an older one.
255. **E-30-04** Artifact trees inherit the same defaults and user overrides;
     the extension shortcut guide reports the effective bindings rather than a
     separately maintained keymap.

## 31. Scrolling stays put

`tests/vm/expectations/31-scrolling.sh`

256. **E-31-01** If I scroll the tree down and another application changes a
     file in a directory I am viewing, the rows I was looking at stay exactly
     where they were.
257. **E-31-02** If I scroll the tree down and press `Shift+R`, the tree
     refreshes and the same rows stay in view.
258. **E-31-03** A thin ruler on the right edge of the tree shows how far I have
     scrolled; it only appears when the tree is taller than the blade, and
     clicking near its top takes me back to the first rows.
259. **E-31-04** Files with a Git status leave a mark on the ruler at their
     position, coloured like their status, so I can see where changes are
     without scrolling. The marks match the statuses the tree actually shows: the
     opened repository's own row displays its summary instead of a status, so it
     leaves no mark and the ruler never counts a change twice.
260. **E-31-05** Marks for rows outside the part of the tree I can see are
     drawn at half strength; marks for rows in view are drawn in full. Turning
     off "Git marks on the scroll ruler" in settings removes every mark and the
     ruler thumb stays.
261. **E-31-06** Extension trees loaded through `ArtifactTree` show the same
     ruler and keep their scroll position when their rows refresh.

## 34. Choosing a monitor

`tests/vm/expectations/34-monitors.sh` uses two outputs in a disposable VM,
including fractional scaling, a gap, a negative origin, and disconnection.
It compares per-output layers, reserved space, real keyboard input and
screenshots, including trash-request cancellation when its owner retires.
Settings pointer checks are exercised separately; delayed-focus and unknown
startup targets also have deterministic QML coverage.

- **E-34-01** With Active selected, a blade opens on the monitor where I invoke
  it and stays there while I work on another monitor. Only its own monitor
  reserves space, and typing still reaches the application I focus elsewhere.
- **E-34-02** Either blade's shortcut closes that blade in one press, even
  while I work on another monitor. The next press opens and focuses it on the
  monitor I am using. A closed blade has no remembered invocation monitor.
- **E-34-03** I can keep the left blade on one monitor and the right blade on
  another. Their sections, tabs, notes and width preferences still belong to
  one shared layout. Opening or closing all blades follows the same rule.
- **E-34-04** A monitor lock makes both shortcuts act on that monitor wherever
  I am working. An unknown lock or explicit ineligible target is rejected
  without redirecting the request or changing my settings.
- **E-34-05** All mirrors the blades on every output. An older Primary setting
  becomes a lock to the first detected monitor.
- **E-34-06** If a locked monitor disconnects, its blades disappear without
  moving elsewhere. The lock remains saved, opening reports no available
  screen, and the blades become eligible when that named monitor returns.
- **E-34-07** If an Active blade's monitor disconnects, the blade does not
  migrate. Its shortcut first closes the unavailable blade; another press
  opens it on the monitor where I am now working.
- **E-34-08** The selection wheel can open on an explicitly targeted output,
  regardless of the blade's monitor lock. A point between or outside outputs
  is rejected without showing an offscreen wheel.
- **E-34-09** Restarting preserves my shared layout and monitor setting.
  Restored open Active blades use the first focused monitor reported after
  startup; a saved lock continues to use its named monitor.
- **E-34-10** Moving focus to another monitor does not transfer or cancel a
  pending trash question. Changing the mode or lock so its owner is no longer
  eligible cancels the question and leaves the files untouched.
- **E-34-11** Settings lists Active, All, and a lock for each detected output.
  Choosing a value changes where blades appear, and Escape dismisses Settings.
- **E-34-12** A delayed navigation result keeps its original target. If I have
  moved to another monitor, it may finish loading data but must not steal
  keyboard focus. No blade chooses an arbitrary output before the focused
  monitor is known.

- **E-34-13** A hover or drop over a visible window on another monitor
  recognizes that window even while I work elsewhere. Explicitly focusing a
  window on another monitor's visible workspace works too.
- **E-34-14** Directional focus reaches a blade only on that blade's assigned
  monitor, including when the current workspace has no windows.
- **E-34-15** Undocking starts a native window on the blade's monitor. I can
  then move it normally; changing focus or monitor settings does not move it
  back to its creation monitor.
- **E-34-16** Unplugging the source monitor during a held module or file drag
  cancels it. The layout and files stay unchanged, and no paste is dispatched.

## 35. Extensions on a shell that hides plugins from each other

This file was written by an agent.

- **E-35-01** On Omarchy 4.0.3, where a plugin is told only about itself, my
  installed FileBlade extensions still appear as tabs with their contents. The
  Welcome tab, the module picker and the settings sheet list them exactly as
  they do on 4.0.2.
- **E-35-02** Disabling an extension with `omarchy plugin disable` removes its
  tab within a few seconds without restarting the shell, and the extension
  stops watching my files. Enabling it again brings the tab back the same way.
- **E-35-03** Installing an extension while FileBlade is running makes it
  available within a few seconds of enabling it. Closing and reopening one
  blade also picks it up, even when my other blade stayed open the whole time.
- **E-35-04** If the list of installed plugins cannot be read, the extensions I
  already have keep their tabs but are shown as unavailable rather than
  silently continuing to run.
- **E-35-05** An extension built before this change still works on a shell that
  discloses plugins to each other, and says it needs an update on one that does
  not, rather than showing an empty tab.

## 36. Explicit cleanup and agent-file management

This file was written by an agent.

Script: `tests/vm/expectations/36-cleanup.sh`. It preserves complete saved
documents and isolates existing Trash/recovery before testing positive policies.
Screenshots name the E number; pending subcases are reported separately.
The current script qualifies the shared Service in harness C at 1920×1080,
with 380/360-pixel blades. It refuses the native shape until R65 supplies
authority routing and an isolated fixture strategy for the sealed payload.
The fixture journals originals and retained scenario data; it does not isolate
Trash stores on other mounts.

- **E-36-01** A fresh install and an existing install without a recorded answer ask
  “Should FileBlade automatically empty the trash?” Never, 1 day, 7 days, 30 days
  and 90 days appear as a list with Never selected. Escape does nothing; Confirm
  is the only completion action, and confirming the initial choice disables pruning.
- **E-36-02** The question blocks FileBlade only. I can use another app and return
  to the same question. It explains shared Trash, permanent deletion and where to
  change the setting. No automatic pruning runs before Confirm is saved.
  It uses FileBlade's font, with the full Trash icon beside the heading, and appears
  once on the left blade, opening it if only
  the right blade was open. An undocked left blade shows it in its own window.
- **E-36-03** Each choice survives restarting the shell without another question;
  changing retention in settings still works. A failed save leaves the question
  present and automatic cleanup off. Config and keybindings record their schema
  and FileBlade release, preserving existing keybindings.

This file was written by an agent.

- **E-36-04** Skills and Memory are browsable and manageable by default, with no
  setup toggle. Removing a fixture places it in the artifact bin; restoring it
  returns the original source bytes.
- **E-36-05** Purging an MCP or Hooks removal deletes its private recovery too.
  Repeating removal and purge does not fill an invisible undo quota. Core recovery
  needs no companion activation. A recovery store that cannot be written refuses
  cleanup while keeping its evidence recoverable.
  If automatic cleanup reaches its scan limit, it reports an incomplete scan
  instead of claiming that every module was checked.
- **E-36-06** Welcome remains built in, dismissible and reopenable, without installing
  companions or changing their existing checkouts. An update check downloads no Git objects
  and does not invent history details when those objects are not available.

## 37. Image galleries and bar popouts

This file was written by an agent.

Script: `tests/vm/expectations/37-image-gallery.sh`, using the Goblins companion
(`kurt.goblin-images`) already staged in the headless VM. The script creates a
24-image fixture from an on-disk library PNG, adds temporary read-only probes,
and restores the guest's source and configuration on exit. The companion
is maintained separately.

- **E-37-01** A module that declares an `icon` shows that picture, tinted like
  its text, in the module picker, the blade settings sheet, its pane header
  and its bar icon; a module without one keeps its glyph.
- **E-37-02** An image gallery groups pictures under month headings, newest
  first, with undated pictures last. Clicking a tile selects it and the
  Properties pane previews it; double-click or Enter opens it with the default
  application; a right-click or `m` opens the same file actions menu as the
  Files tree, naming the picture.
- **E-37-03** `-` and `=`/`+` step the preview size through five sizes; the
  toolbar slider shows the current step with a straight track and filled-centre
  Omarchy mark. Dragging reaches all five stops; arrows and Home/End work while
  focused without losing focus. The choice survives a shell restart in the blade.
- **E-37-04** Typing in the gallery search narrows the tiles live using the
  Files search grammar (`character:bink`, `-tag:danger`); the status shows
  “N of M”; Escape clears the query first and closes the blade only on the
  next press.
- **E-37-05** The right-hand timeline omits periods with no media by default:
  a photo from 2020 and one from 2026 do not leave six years of empty bars.
  I can turn on “Show empty timeline periods” under Media without changing
  which photos are shown. The viewport outline, scrubbing and detail controls
  follow the visible periods, and the header names their real dates so the
  gaps remain clear. A few single-photo periods sit together as selectable
  rows, without bars or stretched gaps. Periods containing several photos
  have bars sized by their counts. The viewport outline appears only when
  the photo grid is taller than the visible pane.
  Drilling from days to hours keeps every day row in the current view and
  replaces its count with vertical bars for the 24 local hours. Hour and count
  labels stay hidden. Clicking a nonempty hourly bar seeks its photos without
  changing the selection; going up restores daily counts. Dates without a
  known time never count as midnight photos.
- **E-37-06** A bar widget that names the module opens the whole module in a
  dropdown under its icon, with the same header, search, chips, grid and
  timeline; Escape or an outside click closes it, and the blade copy of the
  module is unaffected.

## 38. Configuring the drop wheel

This file was written by an agent.

Script: `tests/vm/expectations/38-wheel-config.sh`.

- **E-38-01** If I have no `dropWheel` settings, I get the standard available actions. Version 1 enables configuration; an unsupported version leaves the defaults usable and explains the problem.
- **E-38-02** I can order or hide actions and their placements. Listed available entries come first, unlisted defaults remain, and hiding every built-in placement removes its containing action. I can add custom entries and references to available built-in actions without changing FileBlade source.
- **E-38-03** I can change labels, shortcut keys, glyphs and icons at every supported level. Keys are unique within each ring. An explicit icon or glyph takes precedence over inherited imagery; a built-in reference otherwise retains its original icon and glyph.
- **E-38-04** I can define commands as argument arrays using whole-argument `{paths}`, `{path}`, `{cwd}` and `{git_root}` substitutions. Selected path bytes survive unchanged. An embedded substitution, `{path}` with multiple selections, or an unavailable required value is refused with an explanation.
- **E-38-05** A custom entry appears only for its configured target kinds and when every selected path satisfies its MIME/path conditions. Any pattern in each declared array may match; both arrays must hold when both are present. Unknown MIME does not match. A multiplexer command requires a supported resolved target.
- **E-38-06** I can navigate an action, its placement and a sub-placement using the pointer or keyboard. Letters select entries, arrows/Tab move within the active ring, and Enter accepts. Escape/Backspace returns one layer at a time and then closes the wheel. Containers expose their children and executable leaves run their action.
- **E-38-07** An invalid custom entry is skipped and an invalid override leaves its original entry usable, with a visible footer diagnosis. Valid siblings remain usable. Configuration respects three rings, twelve entries per ring, a ninety-six custom-node budget and the documented argument/document limits.
- **E-38-08** Saving ordinary preferences or editing the wheel preserves unknown members in the surrounding settings and within wheel definitions. Restoring the standard wheel affects only `dropWheel`.
- **E-38-09** Custom commands run detached, in a new terminal, or in a supported herdr/tmux placement with the expected arguments and working directory. Shell-looking path and argument text stays literal; configuration is never treated as an implicit shell command.
- **E-38-10** Before running a configured command or custom built-in reference, FileBlade rereads its settings and file facts. A removed, hidden or newly inapplicable entry cannot execute from an old wheel route.

## 39. Application icons

This file was written by an agent.

Fixture: `tests/vm/fixtures/media_icons.py` with its QML fixture.

- **E-39-01** An Open with row for an application in the installed launcher catalogue shows that application's own icon, resolved from the catalogue's icon name. The row keeps its application identity, and invoking it opens the selected path with that application.
- **E-39-02** An application with a bundled mark and no icon in the desktop theme shows its bundled mark. A generic glyph never replaces a valid bundled mark.
- **E-39-03** When an application's icon file is missing and an explicit override glyph is configured, the override glyph is what I see. The descriptor's own glyph, the legacy fallback glyph and a blank space are all wrong.
- **E-39-04** After an icon file fails to load, a later valid candidate for the same application is shown. Changing the application, its override or its identity clears the remembered failures and lets the new icon load. At most four failed sources are remembered per application.
- **E-39-05** An application icon larger than the space it is drawn in is decoded no larger than 128 device pixels in each dimension, and its aspect ratio is preserved.
- **E-39-06** A place that shows an icon without naming an application keeps its existing icon name, trusted source and fallback glyph behaviour, and looks up no application catalogue.

## 40. Native launch, accepted work and safe shutdown

This file was written by an agent.

`tests/vm/expectations/40-native-authority.sh`
`tests/vm/expectations/48-native-roles.sh`

The first script emits E-40-01 through E-40-04. E-40-05 through E-40-11 are
additional lifecycle expectations; their presence here does not imply that
the section-40 script exercises them. Installed lifecycle and reboot checks
also live in `tests/vm/expectations/95-delivery-installed.sh` and
`tests/vm/native-reboot`. The second script emits E-40-12 through E-40-18
against temporary XDG roots in the installed guest.

1. **E-40-01** If I close FileBlade's view after a move has started, the move
   finishes. Each moved file reaches its destination, and reopening FileBlade
   lets me see the completed result instead of treating the work as cancelled.
2. **E-40-02** Closing and reopening FileBlade's view reconnects to my running
   work. It does not start a second owner of that work or lose its progress.
3. **E-40-03** After closing and reopening the view, I can retrieve a completed
   operation's result, including its source and destination paths. Reading the
   result leaves it available; explicitly collecting it removes that retained
   copy. Closing the view alone does not collect it.
4. **E-40-04** If FileBlade's state folder is moved aside and another folder
   takes its place during a copy, FileBlade reports that it lost ownership.
   It preserves the source and leaves the replacement folder untouched,
   including when I close the view. I can retrieve the failure result.
5. **E-40-05** I can start the native app from its production launcher without
   a development checkout, build tree or spike-home setting. Starting it again
   while it is running opens the existing app with my saved blades and state.
6. **E-40-06** My existing command-line scripts can use `drop-context`,
   `drop-run`, `preferences-read` and `preferences-set` through the native
   launcher unchanged. Multiple paths, negative screen coordinates, empty
   arguments and JSON targets and placement routes keep their meaning.
   `--dry-run` reports the planned action without running it, and changing a
   preference preserves my other settings.
7. **E-40-07** If I request shutdown with a deadline too short for an accepted
   copy, FileBlade reports that it is busy and identifies the unfinished work.
   The copy continues without losing files; after it finishes I can request
   shutdown again successfully.
8. **E-40-08** If I edit ordinary Notes and request a safe shutdown, FileBlade
   saves the changes before exiting. Relaunching restores my edited text and
   blade arrangement.
9. **E-40-09** After a successful safe shutdown, FileBlade's file-chooser
   service also exits. It does not leave an old native process running after
   the app has released its state.
10. **E-40-10** If I request a safe shutdown again after FileBlade has stopped,
    it reports that it is already stopped and succeeds without opening a view.
11. **E-40-11** If FileBlade cannot confirm that its view is ready to exit,
    or I have temporary Notes that have no saved home, an update, removal or
    shutdown request is refused. FileBlade does not report success or force
    the view closed while my unsaved work remains at risk.
12. **E-40-12** After installing, updating or first launching the native app,
    every Desktop integration switch is off. Opening a folder from another
    application, "reveal in file manager" and the file chooser still reach
    the handler I had before, and FileBlade runs beside it.
13. **E-40-13** Turning on "Open folders with FileBlade" in Settings makes
    FileBlade the `inode/directory` handler through the stable launcher. My
    other associations in `mimeapps.list` are left as they were.
14. **E-40-14** Turning on "File chooser" registers FileBlade's portal
    descriptor and service and prefers it for `FileChooser` in
    `portals.conf`, keeping every other portal route.
15. **E-40-15** Turning a role off when I have not changed the handler since
    restores the previous handler byte for byte, removes the files FileBlade
    created, and the row says "restored the previous handler".
16. **E-40-16** Turning a role off after I chose a different handler keeps my
    newer choice, releases FileBlade's ownership, and the row says "kept your
    newer choice".
17. **E-40-17** Turning on "Reveal in FileBlade" while another file manager
    owns `org.freedesktop.FileManager1` still enables the role and the row
    names the current owner and asks me to log out and in. Nothing is killed.
18. **E-40-18** Removing the app runs `native roles disable --all`, which
    reverses every FileBlade-owned entry, leaves the prior handler as it was,
    and reports the exact document the installer requires; a second run
    reports every role already off.

## 41. Native modules and extension popouts

This file was written by an agent.

`tests/vm/expectations/41-native-importers.sh`

1. **E-41-01** In the native app I can open Files, Properties, Notes, Welcome,
   Skills, Memory, Hooks and MCP as blade tabs. Each draws its content without
   an import error or an unavailable-module notice.
2. **E-41-02** With the Goblins companion available, clicking its bar widget
   opens the image module in a popout. Its content loads without an error
   notice, and Escape closes the popout.

## 42. Native section and tab placement

This file was written by an agent.

`tests/vm/expectations/42-native-layout.sh`

The native app follows the same section and tab expectations as section 20.
This script emits the existing E-20-01 through E-20-12 identifiers; it adds
no duplicate E-42 identifiers. Appearance checks retain their screenshot
review requirement.

## 43. Native keyboard focus and desktop space

This file was written by an agent.

`tests/vm/expectations/43-native-parity.sh`

1. **E-43-01** In both plugin and native FileBlade, focusing either blade lets
   me type into that blade without typing into the application behind it.
   Clicking an ordinary application returns keyboard input to that window.
2. **E-43-02** In both shapes, opening a blade reserves its width at the
   corresponding desktop edge. With both blades open, each reserves its own
   side; closing them returns that space to ordinary windows.
3. **E-43-03** If I explicitly hide the desktop bar while both blades are open,
   the bar gives back its space and both blades extend to the top edge.
   The native blades behave like the plugin blades.

## 44. Native blades follow the desktop bar

This file was written by an agent.

`tests/vm/expectations/44-native-bar-state.sh`

1. **E-44-01** With the desktop bar at the top, bottom, left or right, both
   native blades fit beside it. Explicitly hiding the bar extends the blades
   into the freed space; showing it restores the fit. After twenty rapid
   hide-and-show cycles, the final blade position and size still match the
   visible bar state, without a stale gap or overlap.

## 45. Separate chooser windows

This file was written by an agent.

`tests/vm/expectations/45-native-chooser.sh`

1. **E-45-01** When a file chooser opens while I am using a blade, the blade
   releases its keyboard focus so I can use the chooser.
2. **E-45-02** If two file choosers open in different folders, each shows the
   contents of its own folder. One request does not replace the other.
3. **E-45-03** I can keep two chooser windows open in the same FileBlade app
   without starting a separate FileBlade backend for each window.
4. **E-45-04** I can click a different file in each chooser. Each window keeps
   its own selection.
5. **E-45-05** Navigating and selecting files in a chooser leaves my ordinary
   blade's folder, saved state and arrangement unchanged.
6. **E-45-06** Pressing Escape closes the active chooser. The other chooser
   stays open with its selected file intact.

## 46. Choosing files for another application

This file was written by an agent.

`tests/vm/expectations/46-native-chooser-resident.sh`

1. **E-46-01** When an application asks for a text file, I can select a matching
   file but cannot select a file excluded by its filter. Confirming returns
   the selected file to that application while another application's Save
   chooser stays open.
2. **E-46-02** If I choose an existing filename in Save, FileBlade asks me to
   confirm replacement. Confirming returns that destination to the requesting
   application; the chooser itself does not overwrite the file.
3. **E-46-03** When an application asks for a folder, I can select one and
   return that folder to it.
4. **E-46-04** Pressing Escape cancels the active chooser and tells the
   requesting application that I cancelled, without returning a selection.
5. **E-46-05** If the requesting application exits before I finish choosing,
   its chooser closes instead of leaving an orphaned window.
6. **E-46-06** Finishing chooser requests leaves my ordinary blade's folder,
   saved state and arrangement unchanged.
7. **E-46-07** When multiple selection is allowed, I can select two files with
   Ctrl-click and return both files to the requesting application.
8. **E-46-08** In Save I can choose a filename that does not exist yet. The
   requesting application receives that destination; choosing it alone does
   not create the file.

## 47. Uploading through the native file chooser

This file was written by an agent.

`tests/vm/expectations/47-native-portal.sh`

This scenario temporarily selects FileBlade for the guest's file-chooser
portal and uses an actual Chromium upload page. It checks the installed app
when `FILEBLADE_SHAPE=native`; desktop-role switches and cold D-Bus
activation require their separate integration checks.

1. **E-47-01** When I open a browser's upload chooser and press Escape,
   FileBlade closes that chooser and the browser receives cancellation.
   No file is uploaded.
2. **E-47-02** When I select a file in FileBlade's chooser and confirm it,
   the browser receives that file's name and size. Uploading sends the
   selected file's exact contents.
3. **E-47-03** Selecting FileBlade for file choosing leaves my other portal
   routes unchanged. After the scenario, my prior chooser routing, blade
   arrangement and ordinary Notes are restored.

## 48. Agent usage history and activity heatmap

This file was written by an agent.

No VM script covers this section yet. The parts that run without a desktop are
checked by `tests/qml/tst_usage_heatmap.qml` (weeks by width, colours, tooltip
text, keyboard cursor), `tests/qml/tst_artifact_inventory.qml` (history
requests), `tests/qml/tst_usage_modules.qml` (both modules' focus, visibility,
errors and MCP Right/`l` expansion), `tests/qml/tst_artifact_branches.qml` (servers from one file fold
separately), `tests/core_modules_usage_golden.rs` (counting and history) and
`tests/usage_cli_e2e.rs` (the `fileblade usage` commands). Placement under the
search field and compositor focus still need a VM scenario at blade widths 280 and 1000, with the search
field both shown and auto-hidden. See
[agent usage history](../docs/agent-written/agent-usage.md) for the counting rules.

1. **E-48-01** When I open a Skills or MCP tab, the search field is the first
   row, the tab header with its columns and buttons is under it, and a grid of
   small square cells sits between that header and the list. Each column is a
   week, and its seven rows start on my locale's first day of the week. The
   newest week is the rightmost column, today is its last cell, and days after
   today are not drawn. When the search field is auto-hidden, the header moves
   up to the top and the grid stays directly under it. The header has no
   Search button; `/` reveals the field.
2. **E-48-02** When I widen the blade, older weeks appear on the left; when I
   narrow it, the oldest weeks leave from the left. Cells that stay on screen
   keep their colour. Width too narrow for another week stays empty on the
   left, and the grid never shows more than 160 weeks.
3. **E-48-03** The busier a day, the stronger its cell in the accent colour, in
   four steps set by how busy my active days are across the whole loaded
   history. A day with no use has a faint tint. A day before FileBlade's history
   begins has no fill at all.
4. **E-48-04** Hovering a cell shows a tooltip such as
   `Mon 14 Sep 2026: 6 skill uses (4 agent, 2 you, 1 failed)`, with day and
   month names from my locale. The MCP tab says `MCP calls`, and a single use
   reads `1 skill use` or `1 MCP call`. Parts that are zero are left out.
   Commands a scheduled task ran appear as `N scheduled` and are not part of
   the total. A day with no use says `no skill uses` or `no MCP calls`; a day
   before the history begins says `no history yet`.
5. **E-48-05** I can move keyboard focus from the search field onto the grid
   with Tab. An accent outline marks today, and the tooltip shows for the
   outlined day. Left and Right move the outline a week, Up and Down a day,
   Home jumps to the first visible day and End to today; the outline never
   leaves the visible days. A screen reader reads the same text as the tooltip.
   Tab or Escape returns keyboard focus to the list. Shift-Tab returns to
   search, revealing it when automatically hidden.
6. **E-48-06** Skills and MCP tab headers have an Activity button with a
   calendar glyph. Clicking it hides the grid and the list moves up into the
   space; clicking it again brings the grid back. The choice belongs to that
   tab, starts on for a new tab, and survives a shell restart. Files tabs have
   no Activity button.

   In Skills, selecting a skill with the mouse or keyboard scopes the visible
   heatmap to that skill, including its agent and typed uses. Selecting a file
   inside that skill retains the scope; selecting a group returns to all skills.
   Clicking a day or pressing Enter on it filters the tree to skills used that
   day without rescanning the skill directories. Switching days keeps the
   current filtered rows until the next result arrives. The selected square
   has an inset foreground-colored border, and all other squares are dimmed;
   the border remains visible after focus returns to the tree. Clicking the
   selected day again or pressing Escape in the tree clears the day filter,
   border and dimming. Changing projects clears the old selection and filter.
7. **E-48-07** When the tab's section is shorter than about 300 pixels, the grid
   hides by itself and the list takes the space. When the section grows again,
   the grid returns, and my Activity choice is unchanged.
8. **E-48-08** "No history yet" and "no use" are different things. The history
   begins at the earliest transcript record FileBlade has ever read. Deleting
   old transcripts, by hand or through the agent's own cleanup, never moves
   that beginning later. Before any transcript has been read, no cell is filled
   and every day says `no history yet`.
9. **E-48-09** Uses survive deleted transcripts. After an agent deletes an old
   transcript, the Uses, Uses (agent) and Uses (user) columns and the grid still
   count the uses it held. Counts only go down when I run `fileblade usage forget`.
10. **E-48-10** Skills Uses counts an agent calling a skill and me typing
    `/<skill>`. A plugin skill also counts calls recorded as `<plugin>:<skill>`.
    A command run by a scheduled task is not in Uses, and a slash command that
    is not a skill, such as `/clear`, counts for nothing. A command copied into
    a resumed session counts once. A typed command counts the same no matter
    which tab reads the transcript first.
11. **E-48-11** An MCP server row counts calls under the name the agent recorded
    for it. A Claude Code server configured as `my.server` counts its
    `mcp__my_server__…` calls, a plugin server counts its
    `plugin_<plugin>_<name>` calls, and a Codex server counts Codex's calls to
    it. When two servers of one agent would be recorded under the same name,
    such as `twin.a` and `twin_a`, both show 0 instead of a guess. Servers of
    other agents show 0.
12. **E-48-12** An MCP server row with recorded use has a fold marker. Expanding
    it lists every tool, resource, resource list and prompt the agents used
    through that server, most used first. Each child has its own glyph, its
    count in the Uses column, and `N failed` in its detail when any call
    failed; a resource list reads `resource list`. Children have no actions
    and no menu. Two servers read from the same configuration file expand and
    collapse separately. Enter or a double-click expands such a row, and `o`
    still opens its configuration file.
13. **E-48-13** The first time a large transcript history is read, the counts
    and the grid can be partial, newest transcripts first; a visible grid
    automatically fills in the rest. Skills and MCP tabs open at the same time never
    count a use twice.
14. **E-48-14** `fileblade usage skills` prints one `YYYY-MM-DD<TAB>uses` line
    per local day with any skill use. Typed commands count for the skills
    visible from the directory I run it in. `fileblade usage mcp` prints the
    same for MCP calls. Neither prints anything when there is no use. With
    `-o json`, both print the whole history document. Both work while the shell
    is stopped.
15. **E-48-15** `fileblade usage forget --before 2026-06-01` deletes the history
    of local days before 1 June 2026 and prints `removed N`; the grid then says
    `no history yet` before that day. `fileblade usage forget` deletes all of
    it. Transcripts FileBlade already read are not counted again afterwards. A
    date that is not a real zero-padded `YYYY-MM-DD`, such as `2026-6-1` or
    `2026-02-30`, is refused with exit status 2 and nothing is deleted.

This file was written by an agent.

- **E-48-16** A first open with a large history fills in automatically until
  reading finishes. The header says “Reading activity…” while more remains.
  A failed activity request shows its error in the header and keeps the last
  good grid. Refresh retries it.
- **E-48-17** Hidden activity does not request daily history. Turning Activity
  back on or growing a short section requests it again. Hiding the grid while
  it has keyboard focus returns focus to the list.
- **E-48-18** Right or `l` on an MCP definition with observed children expands
  it without opening configuration; pressing again moves to its first child.
- **E-48-19** Forgotten history stays forgotten when an old transcript is
  replaced, shortened or copied, or an older unread transcript is found. A
  full forget also excludes history dated through that moment; later uses
  still count. Forget reports the number of stored events removed.
- **E-48-20** Skill Uses count every agent FileBlade manages. A Codex `$skill`
  mention counts as a use by me; a Codex, Antigravity or Pi read of a skill's
  `SKILL.md` counts as one agent use per turn; an OpenCode `skill` tool call
  counts once it completes; a Copilot CLI skill counts as mine when I invoked
  it and as the agent's when it did. A Copilot or OpenCode MCP call counts for
  its server row.
- **E-48-21** While an agent is running in another window, the Uses column and
  the heatmap of an open Skills or MCP tab climb on their own within a few
  seconds of the agent writing its transcript, without me pressing refresh.
  When the tab is closed nothing is read.
- **E-48-22** A skill I disabled stays in the list, struck through, from the
  moment the dialog closes, after a rescan, and after the shell restarts. It
  never waits for another disable to appear.
- **E-48-23** Right-clicking a disabled skill opens the same menu as any other
  skill. Open, Reveal and the file actions act on the disabled copy FileBlade
  keeps; Enter and the row's action button still offer Restore and Delete
  forever.
- **E-48-24** The row's Delete button asks Cancel, Delete skill, then
  Deactivate. When the skill is a symlink, Delete symlink sits between them:
  Delete skill trashes the folder the link points at, Delete symlink trashes
  only the link, and Deactivate moves the link into FileBlade's bin with the
  target untouched. Delete forever on a deactivated symlink reads Delete
  symlink forever and never touches the target. Memory, Hooks and MCP rows use
  their own noun in the same dialog.

## 49. Branches

This file was written by an agent.

No VM script covers this section yet. The rows, status text, search, switch
and worktree navigation are checked by `tests/qml/tst_branches_module.qml`
on a fixture document; the slot position by
`tests/qml/tst_layout_inventory.qml`; first-use and saved layouts by
`tests/core_modules/run-slots`; the backend document and CLI by
`tests/git_places.rs`. The QML module suite also runs under Wayland in the
isolated guest for rendering and keyboard/pointer checks.

1. **E-49-01** When I click the branch name in the tree footer, the Switch
   branch popup opens with a first row `Expand into Branches` above a
   separator and the branch names. Picking it closes the popup and a
   Branches tab appears beside Properties, in whichever blade Properties
   sits, and becomes the active tab. Without a Properties slot it takes its
   own pane under Files at about a third of the height. The blade opens if
   it was closed and the pane takes focus.
2. **E-49-02** `fileblade branches` opens or focuses the same pane;
   `fileblade branches close` removes it and closes the blade when nothing
   else is left in it. On first opening FileBlade, Branches is already the
   second tab behind Properties, with Properties selected. My saved layout
   remains authoritative: closing Branches does not make it return on restart.
   It is also listed in Add module.
3. **E-49-03** The pane lists a Branches group, a Remote subgroup for
   branches that exist only on a remote, then a Worktrees group for detached
   worktrees. The main checkout is never listed as a worktree; its branch
   carries the check mark. A linked worktree sits nested under the branch it
   has checked out, open by default, showing the folder name of the worktree
   with its path as summary, a folder glyph, and a separate check mark
   when it is the one I am in. A branch row shows the branch name, the
   tip commit's subject as its summary, a blue theme-accent branch glyph for
   local branches or a dim cloud glyph for a remote-only branch, and a
   separate check mark on the checked-out branch.
4. **E-49-04** The Status column, shown by default, uses the same layout and
   colours as the repository summary on the tree's root row: `↑3 ↓1 M4 A1 ?2`
   for a branch with an upstream and a checkout with changes, following the
   Git summary fields setting. A branch checked out nowhere shows only its
   arrows, `↑0 ↓0` when in sync; a nested worktree row shows its changes with
   `locked` appended when locked; `clean` when nothing differs, `no upstream`
   for a local branch without one, `gone` when the upstream was deleted, and
   `remote` for a remote-only branch. Kind (`local`, `remote`, `both`),
   Updated (the tip commit's date), Author and Summary are the other column
   choices.
5. **E-49-05** Branches come first, by most recent commit, then detached
   worktrees. Typing in the filter field matches branch names, subjects,
   authors, upstreams, kinds and worktree paths; `kind:remote` and
   `remote:origin` narrow by field. The header shows the current branch and
   its status with a Git branch glyph, or `2 of 7` while filtering. Without
   a current branch it counts branches and linked worktrees.
6. **E-49-06** Selecting a checked-out local branch or a worktree selects its
   folder in the Files tree when it is already listed; otherwise Files opens
   that folder. Mouse and keyboard selection behave alike. Selecting a
   remote-only branch does not navigate. Enter or a double click on a branch
   without a checkout switches the repository to it, also when it exists
   only on a remote. A refusal such as a dirty
   working tree shows in red in the header and nothing changes. On success the
   list reloads, the check mark moves, and the tree's Git markers and summary
   chip refresh. Enter on a worktree row, nested or detached, opens that
   folder in the tree; Enter on a branch with a nested worktree folds or
   unfolds it; `o` on a branch checked out in another worktree opens that
   worktree instead of switching.
7. **E-49-07** The list reloads when I open the pane, when the tree moves to
   another repository, after the tree's own Git refresh, and on Shift+R.
   Closely spaced refreshes run once; closing or changing context cancels
   pending work and late replies cannot replace the current repository. It
   never polls. Outside a repository the pane says `no repository here`.
   Escape closes the blade, `/` opens the filter, `f` the column filter and
   `s` cycles the sort, as in the other panes.

## 90. Checking the native app before installation

This file was written by an agent.

Script: `tests/vm/expectations/90-delivery-payload.sh SOURCE BACKEND TARGET NOTICES`,
run inside the assigned VM. These checks inspect the staged app; opening it and
using its modules are separate installed-app expectations.

- **E-90-01** When I check a complete native app for a supported system with its
  required software available, the check succeeds. The app includes its runtime
  files, notices, keyboard reference and extension reference.
- **E-90-02-digest** If an app file has changed since the package was prepared,
  checking it fails instead of accepting the changed copy.
- **E-90-02-mode** If an app file's access permissions differ from those recorded
  for it, checking the app fails.
- **E-90-02-extra** If the app contains an unrecorded file, checking it fails.
- **E-90-02-missing** If a recorded app file is missing, checking it fails.
- **E-90-02-symlink** If a recorded app file has been replaced with a link to a file
  elsewhere, checking it fails.
- **E-90-03-duplicate** If the app's file list records the same file twice,
  checking it fails.
- **E-90-03-traversal** If the app's file list names a path outside its own
  directory, checking it fails.
- **E-90-04-architecture** If the app's recorded processor architecture does not
  match its executable, checking it fails.
- **E-90-04-abi** If the app's recorded Linux runtime type does not match its
  executable, checking it fails.
- **E-90-05-missing-dependency** If a required command is unavailable on my system,
  checking the app fails and names the missing command.
- **E-90-06** After I restore the app's original files, permissions and file list,
  checking it succeeds again.
- **E-90-07-find-verify** If checking the app cannot finish reading its directory,
  it reports failure rather than calling a partial check successful.
- **E-90-07-find-stage** If preparing the app cannot finish reading its directory,
  it fails without publishing an incomplete app.
- **E-90-07-sort-verify** If checking the app cannot finish ordering its file list,
  it reports failure rather than accepting an incomplete list.
- **E-90-07-sort-stage** If preparing the app cannot finish ordering its file list,
  it fails without publishing an incomplete app.
- **E-90-08** The installed app does not include the source repository's root
  `AGENTS.md`, and its file list does not claim to include that file.

## 91. Installing and recovering a user-local app

This file was written by an agent.

Script: `tests/vm/expectations/91-delivery-install.sh PAYLOAD`, run inside the
assigned VM. The script uses temporary home directories, controlled update
copies and process interruptions. Its settings and Notes checks cover files
already saved on disk; they do not prove that unsaved edits survive shutdown.

- **E-91-01** If an unrelated command already occupies my `fileblade` launcher
  path, installation refuses to overwrite it and leaves its contents intact.
- **E-91-02** I can install FileBlade in my own home, including when its path
  contains spaces. The launcher points to the installed app, and installing the
  same app again keeps that version active.
- **E-91-03** Installing another version makes it active while keeping the
  previous version recoverable. Rolling back selects that previous version.
- **E-91-04** While the installed app is still in use, I can inspect its status,
  but an update cannot replace the active version until it is safe to do so.
- **E-91-05** If installation stops during copying or just before or after
  switching versions, the selected app remains complete and agrees with its
  installation record. I can retry the installation and roll back afterward.
- **E-91-06** Installing, updating, retrying an interrupted installation and
  rolling back preserve my saved settings and Notes files.
- **E-91-07** If the installation record no longer identifies FileBlade as its
  owner, or its owned launcher has changed, an update refuses to replace the
  installation.
- **E-91-08** If the installation directory is a link to an unrelated directory,
  installation refuses and leaves that directory and its files untouched.
- **E-91-09** An update with a different dependency contract is refused even
  when its own dependency check succeeds. My current version stays selected,
  and I can still roll back and return to it.
- **E-91-10** An update whose dependency contract changed installs when its
  `packaging/runtime.json` lists the digest of my current contract under
  `upgrades` and its own dependency check succeeds. I can roll back to the
  previous version afterwards.
- **E-91-11** If a downloaded archive contains a different version or processor
  target from the advertised release, installation stops before its installer
  runs and leaves my installed app unchanged.
- **E-91-12** Preparing release archives in a directory for an older version
  refuses to mix the two versions and preserves the existing release files.
- **E-91-13** With a home path containing spaces, quotes, backslashes, dollar
  signs, backticks or percent signs, enabled desktop entries and D-Bus services
  still launch the intended FileBlade command with its arguments intact.

Local regressions: `cargo test --locked --test install_bootstrap --test native_roles`.

## 92. Installing and removing the Arch package

This file was written by an agent.

Script: `tests/vm/expectations/92-delivery-package.sh SOURCE PAYLOAD`, run inside
the assigned VM with package-management access. It uses real package
transactions and checks the protection against starting FileBlade during them.

- **E-92-01** The Arch package contains the same verified app I supplied and
  declares its required software. Preparing it from a private directory does
  not leave the packaged app accessible only to its owner.
- **E-92-02** After installation, the package manager recognizes the FileBlade
  launcher and executable as belonging to the package. The installed app matches
  the supplied copy, and I can request help through its launcher when idle.
- **E-92-03** If package-owned FileBlade files coexist with my user-local install,
  the direct installer refuses to update or remove them and tells me to use
  `pacman`. Both installations stay intact.
- **E-92-04** Removing the Arch package prevents a new FileBlade launch during
  removal and removes its packaged commands and app files. My separate user-local
  installation, file-manager preference and chooser preference remain intact.
- **E-92-05** If the packaged app is still in use, package removal refuses to
  proceed and leaves the package installed.
- **E-92-06** If I try to launch FileBlade after a package upgrade has checked
  that it is idle but before the transaction finishes, the launch is refused
  with a request to retry afterward. The upgrade preserves the verified app.

## 93. Removing an owned app and recovering interruptions

This file was written by an agent.

Script: `tests/vm/expectations/93-delivery-remove.sh PAYLOAD`, run inside the
assigned VM. The script changes a temporary installation and interrupts removal.
Its personal-data checks cover saved files, not unsaved Notes or desktop-role
restoration.

- **E-93-01** If the active app's files are missing, rollback refuses to guess
  which installation is safe. Restoring the verified files lets me roll back.
- **E-93-02** If the record selecting my active installation is missing, another
  install refuses to overwrite its history. Restoring that selection lets me
  install again.
- **E-93-03** Removal refuses to delete an installation whose launcher has changed
  or whose app directory contains an unrecorded file. Those changes remain intact.
- **E-93-04** Removal refuses to delete the app while it is still in use.
- **E-93-05** If removal stops after deleting only part of the app, another install
  refuses until removal finishes. Retrying removal completes it, removes the launcher,
  retains the removal record and preserves my saved Notes and desktop defaults.
  Repeating the completed removal succeeds harmlessly.
- **E-93-06** After removing FileBlade, I can install it again and remove that
  installation successfully.
- **E-93-07** After I remove one installation and install another, the older
  removal record cannot stand in for a missing current installation record.
  Another install or removal refuses, rather than overwriting the newer app or
  reporting that it has already been removed.

## 94. Waiting for a safe update or removal

This file was written by an agent.

Script: `tests/vm/expectations/94-delivery-lifecycle.sh SOURCE PAYLOAD`, run inside
the assigned VM. A temporary launcher supplies controlled maintenance responses;
this proves how the installer handles them, not that real work or unsaved Notes
finish safely. The script also runs sections 91 and 93 against that fixture.

- **E-94-01** If removal cannot confirm that FileBlade has reversed its managed
  desktop roles and finished its running work, it refuses to delete the app.
  The active installation record stays unchanged and the launcher remains available.
- **E-94-02** If the selected installation changes while an update is waiting
  for FileBlade to stop, the update refuses to continue against a different
  installation and explains that the selection changed.
- **E-94-03** When I remove FileBlade, removal requests reversal of its managed
  desktop roles before asking the app to finish its running work and stop.

## 95. Using an installed app through update, rollback and removal

This file was written by an agent.

Script: `tests/vm/expectations/95-delivery-installed.sh PHASE PAYLOAD`, run inside
the assigned VM. The phases are `install`, `check`, `update`, `rollback` and
`remove`; each receives the app expected to be active afterward, or the app being
removed. These checks use actual app payloads and their installer. Run the scoped
UI expectations between installation and update; this script alone does not prove
UI behavior or preservation of unsaved Notes.

- **E-95-01** When I install FileBlade into a profile with no active installation,
  the requested app becomes active, its files verify successfully and my stable
  launcher points to that installation.
- **E-95-02** When I check the installed app, its installation record and launcher
  identify the expected app, and its complete file inventory verifies successfully.
- **E-95-03** When I update FileBlade to a different app payload, that app becomes
  active and verifies successfully. The installation record keeps the identity of
  the app I was using before the update.
- **E-95-04** When I roll back, the expected previous app becomes active, its files
  verify successfully and the same launcher points to the restored installation.
- **E-95-05** When I remove FileBlade, the active and previous app files and the
  stable launcher are removed. The removal record preserves the installation
  record I had before removal.

## 96. Using the native VM harness without changing commands

This file was written by an agent.

Script: `tests/vm/expectations/96-delivery-adapter.sh ADAPTER`, run inside the
assigned VM. A temporary recording command checks forwarding and refusals; it
does not install an app or exercise the real desktop.

- **E-96-01** If I send an SSH command through the native harness, empty values,
  spaces, newlines, quotes and shell-looking text reach the underlying harness
  unchanged. I receive the same success or failure status.
- **E-96-02** Requests to ordinary desktop IPC targets keep their arguments
  unchanged and return the underlying harness's success or failure status.
- **E-96-03** Status, fresh start, stop and screenshot requests keep their
  arguments and return the underlying harness's success or failure status.
- **E-96-04** If I have not explicitly enabled pushing, a push request is refused
  before any command reaches the VM.
- **E-96-05** With `SKIP_PUSH=1`, a push request is refused before any command
  reaches the VM.
- **E-96-06** If I accidentally select the native adapter itself as its underlying
  VM harness, the request fails promptly instead of repeating indefinitely.

## 97. Controlling the installed app through the native VM harness

This file was written by an agent.

Script: `tests/vm/expectations/97-delivery-adapter-runtime.sh ADAPTER`, run inside
the assigned VM. A temporary installation and controlled launcher responses
check routing, restart ordering and refusals. Actual backend effects and safe
shutdown still require the installed-app and UI scenarios.

- **E-97-01** When I send a native IPC request, its arguments reach the selected
  installed app unchanged, and the request uses that app's matching state location.
- **E-97-02** When I use `drop-context`, `drop-run`, `preferences-read` or
  `preferences-set` through the adapter, the command and all its arguments reach
  the installed app's backend entry point unchanged.
- **E-97-03** Both `restart` and `restart-shell` ask the app to finish its work
  before launching it again. Restart succeeds once the installed app and its
  bundled modules are ready, including when an additional extension is present.
- **E-97-04** If the app reports that it is busy, returns an unrecognized shutdown
  result, or the selected installation changes during shutdown, restart fails
  without launching another app. A busy refusal returns status 3.
- **E-97-05** If the installed app's executable is missing, a native status
  request fails before attempting to launch the app.

## 98. Seeing how full the drive is

This file was written by an agent.

Script: `tests/vm/expectations/98-capacity-bar.sh`. The script mounts its own
64 MiB loop image, a bind mount of it and a tmpfs stacked inside it, and
removes them when it ends. The home folder's mount is read from `findmnt`,
never assumed.

- **E-98-01** With the home folder open, a thin blue bar under the file toolbar
  shows a fill, no wider than the blade, and the bar names the mount the home
  folder is on.
- **E-98-02** Hovering the bar shows how much of the drive is used out of how
  much, the percentage full and the free space, in MB, GB or TB.
- **E-98-03** Opening the loop image names its mount and the fill shrinks to that
  image's fullness. A bind mount of it names the bind target with the same
  numbers. A mount stacked inside it reports itself, and once unmounted the
  same folder reports the image again.
- **E-98-04** Trash, Recent and Drives show no fill; only the plain hairline is
  left under the toolbar.
- **E-98-05** `fileblade space PATH` reports the used, free and total bytes
  byte for byte as `df -B1` does, and df's percentage; the text form prints one
  line with that percentage.
- **E-98-06** `fileblade space` without a path measures the open folder, and
  refuses while Trash is open.
- **E-98-07** Copying a 16 MiB file into the image through FileBlade raises the
  fill within ten seconds of the copy finishing.
- **E-98-08** Turning "Drive usage under the toolbar" off in Files settings hides
  the fill at once and leaves the hairline; the choice survives a shell restart;
  turning it back on brings the fill back.
- **E-98-09** Clicking the bar does nothing, and the toolbar buttons just above it
  keep their own tooltips and still work.
- **E-98-10** Switching between two folders on different drives within a second
  ends on the last folder's mount and stays there.
- **E-98-11** After the backend restarts, the bar asks again on its own and shows
  the fill. Removing the open folder moves the tree to its parent, with the
  parent's fill. Unmounting the image under the open folder reports the
  filesystem left behind.
- **E-98-12** A file written into the image by another program shows as a larger
  fill within the minute.
- **E-98-13** Right after the image is mounted, its row in the Drives list draws
  a bar as full as the backend's fraction says, within two pixels of its own
  track, and the backend reports df's percentage for it.
- **E-98-14** After a theme switch, the fill takes the
  theme's blue when the theme defines one, and falls back to blue when the
  theme has none. FileBlade's background, text, accents and folder palette
  follow the new theme without a restart, including switching back to a
  previously used theme.
- **E-98-15** Opening a symlink to a folder on the image keeps the symlink in the
  location while the bar reports the image.
- **E-98-16** Closing the blade or collapsing its section reports the bar hidden
  and stops the minute refresh; reopening or expanding resumes it. Mirrored
  outputs share one tab state, so their bars agree; the single-output guest
  does not exercise that.
- **E-98-17** When the open folder cannot be read, the next refresh turns the bar
  into the plain hairline, and once the folder is readable again the fill
  returns on its own within half a minute. The busy answer of an overlapping
  probe, including after the first request's deadline, is proven in
  `tests/capacity_e2e.rs` against one resident backend.

This file was written by an agent.

- **E-40-19** Native FileBlade's desktop bindings reach the standalone app through its stable
  launcher. Super+B opens or focuses the left blade, Super+Shift+B the right,
  and Super+Z undoes the last file operation without opening quick navigation,
  even when the legacy host plugin is absent.
  Repeated binding installation does not leave duplicate Super+B actions.
- **E-40-20** A generated extension recognizes a running native FileBlade without the old
  host plugin in Omarchy's catalogue. If installed but stopped, it asks me to
  start FileBlade; it does not claim FileBlade is missing or restart Omarchy.
- `fileblade doctor` reports the current folder. Native failures advise starting
  FileBlade, while legacy-plugin failures retain the corresponding shell advice.
- If startup recovery is blocked, `fileblade doctor` reports an unhealthy
  runtime, exits unsuccessfully and includes the recovery failure in its advice,
  even when the backend and view answer requests.
- Native CLI backend helpers, including drive capacity and archive extraction,
  use the running authority. They refuse cleanly when that owner is stopped.
- **E-40-21** The native `fileblade preferences` command reads and saves through
  the running persistence service. Changes survive the next read and a stopped
  service cannot cause a separate writer to modify the settings.

This file was written by an agent.

- **E-40-22** Starting native FileBlade while Omarchy is still loading waits for
  plugin discovery before checking for old writers. Once the shell is ready,
  keybindings load without a legacy-writer warning. A recovered backend clears
  a previous keybinding error automatically.
- **E-07-09** Hovering the repository summary above the file tree shows branch,
  upstream comparison and file-change details without selecting the root folder.
