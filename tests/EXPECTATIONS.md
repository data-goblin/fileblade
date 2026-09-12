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

49. **E-07-01** When I show hidden files, each hidden file has a crossed-out-eye
    icon that identifies it as hidden.
50. **E-07-02** If I press `.`, `Shift+H`, or `Ctrl+H`, hidden files toggle
    between shown and hidden.
51. **E-07-03** When I view a hidden file, its row is muted but I can still see
    its real Git status.
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
147c. **E-17-14** If I set Drag out to System drag in the Files settings,
     dragging rows hands the files to the window I drop on, so an application
     that takes files receives them instead of their typed paths, and the drop
     wheel stays out of that drag unless I drag with my right button;
     Paste path restores the old behavior, and the choice survives a restart.

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
     blade with an arrow pointing that way: `→` "Move to right blade" from the
     left blade, `←` "Move to left blade" from the right. Choosing it takes the
     tab out of its section and places it as the top section of the other
     blade, opening that blade if it was closed, and focus follows the tab.

## 23. Opening files, locations and recent items

`tests/vm/expectations/23-opening.sh`

191. **E-23-01** If I select a file and press `Enter` or `o`, it opens in its
     default application. On a folder, `Enter` toggles expansion in place and
     `o`, `l` or Right enters it as the tree root; `h` or Left goes to the
     parent directory.
192. **E-23-02** If I select a file and press `Shift+Enter`, I can choose from
     compatible applications and open the file with my choice.
193. **E-23-03** If I select one file and press `e`, it opens in LazyVim.
194. **E-23-04** If I press `Enter`, `Shift+Enter`, or `e` from Properties, the
     selected item opens the same way it does from the file tree.
195. **E-23-05** If I press `r` from Properties, my file manager opens with the
     selected item revealed.
196. **E-23-06** If I press `Ctrl+L`, I can type or paste a location and open it;
     an invalid location shows an error without moving me elsewhere.
197. **E-23-07** If I click Recent, I see files and folders I opened recently,
     ordered so useful recent choices are easy to reach.
198. **E-23-08** If I choose an item from Recent, it opens normally and remains
     available in my recent history.
199. **E-23-09** If I have no recent items, the Recent view tells me so instead of
     showing a blank or broken list.
200. **E-23-10** If I press `Ctrl+P`, I can use the picker to find files, recent
     items, content matches, or actions, and its visible prefixes tell me which
     mode I am using.
201. **E-23-11** If I double-click a file, it opens in its default application;
     double-clicking a folder opens it as the tree root, whether it is collapsed
     or expanded. `Enter` and the disclosure arrow still expand it in place.
202. **E-23-12** If I double-click or press `Enter` on a file listed by a companion
     module such as Memory, Skills, Hooks, or MCP, it opens in the same default
     application it would open in from the file tree, not in the terminal editor.

## 24. Search syntax, columns and colors

`tests/vm/expectations/24-search-columns-colors.sh`

203. **E-24-01** When I search, I can use quoted text for an exact phrase,
     `-word` or `!word` to exclude text, and `^start` or `end$` to anchor a match.
204. **E-24-02** When I search, I can narrow results with `type:`, `format:`, and
     `in:` filters.
205. **E-24-03** If I search with `content:`, I see matching lines grouped under
     their files; if I use `scope:everywhere`, I can find matching paths beyond
     the current folder.
206. **E-24-04** If I press `Ctrl+Shift+B` during a deep search, the results
     switch between a flat list and a tree without changing the search.
207. **E-24-05** If I click the add-column control, I can choose another detail
     to show beside each row; I can also remove a column I no longer want.
208. **E-24-06** If I drag a detail column, it moves to the indicated horizontal
     position without obscuring the other column labels.
209. **E-24-07** If I click a column label or use its menu, I can sort ascending,
     descending, add it as a secondary sort, or clear sorting; numbered arrows
     show the order of multiple sorts.
210. **E-24-08** If I filter a date or number column, only matching rows remain
     and a visible filter marker stays until I clear the filter.
211. **E-24-09** If I close and reopen a blade, my chosen columns, their order,
     sorting, and filters are preserved.
212. **E-24-10** If I choose which Git details to show, the Git status column
     displays those details without replacing the item's normal file icon.
     Counts use the same marker-first notation as the repository summary;
     the ? count includes only untracked files, never staged additions.
213. **E-24-11** If I color one or more files or folders, I can choose a palette
     color or enter a six-digit custom color and see it applied immediately.
214. **E-24-12** If I change the color scope, the color applies only to the icon,
     the name, or the whole row as selected; Reset returns to the theme color.
215. **E-24-13** If I open a column's menu, it has a heading (Columns, Add
     column, or Git status) with an `×` that closes it, above the filter field.
216. **E-24-14** The folder colour swatches never match a Git status colour;
     the red, yellow and green swatches are visibly rose, lemon and mint next to
     the amber, green and red Git markers. If I write my own hex colours into
     `colors.json` in FileBlade's config folder, the swatches and every coloured
     row take them without a restart; a bad value is ignored and the previous
     palette stays.

## 25. Notes

New notebooks begin with the supplied Lovecraft quotation. Existing notes,
including intentionally empty notes, retain their saved contents.

`tests/vm/expectations/25-notes.sh`

217. **E-25-01** If I add the Notes module, I can type a plain-text note directly
     in the blade.
218. **E-25-02** When I stop typing, switch modules, collapse Notes, or close the
     blade, my note is saved without a separate Save action.
219. **E-25-03** If I click `+` in Notes, a new named note tab appears and I can
     keep different text in each note. Default note numbers keep increasing
     after I close notes or restart, so deleting Note 1 does not create
     another Note 2.
220. **E-25-04** If I switch note tabs, each note keeps its own text and the tab I
     chose is visibly active.
221. **E-25-05** If I double-click a note tab or choose Rename from its menu, I
     can rename that note.
222. **E-25-06** If I middle-click a note tab or choose Close note, that note
     closes; FileBlade keeps at least one note so I cannot lose the notebook
     itself by closing the last tab.
223. **E-25-07** If I close and reopen the blade or restart the shell, my note
     names, text, and active note are preserved.
224. **E-25-08** If my notes reach their storage limit, FileBlade warns me and
     stops accepting excess text instead of silently losing saved content.

## 26. Dragging files outside a blade

`tests/vm/expectations/26-drop-wheel.sh`

225. **E-26-01** When I drag files outside a blade, a compact ghost follows the
     pointer with the last grabbed row's icon and name.
226. **E-26-02** If I drag several selected items, the ghost shows how many items
     I am carrying while still naming the last grabbed row.
227. **E-26-03** If I hold my configured drop-wheel modifier during the drag, a
     wheel opens at the pointer and shows actions for the items I am carrying.
228. **E-26-04** When I drag over an application, editor, terminal, or empty
     desktop, the wheel offers actions that make sense for that target.
229. **E-26-05** I can move around the wheel with the pointer, scroll wheel,
     arrow keys, `h/j/k/l`, or `Tab`, and the highlighted choice is always clear.
230. **E-26-06** If an action offers placements such as a tab, split, pane, or
     window, I can enter that second ring and choose the exact destination.
     The Herdr and tmux opening actions offer horizontal and vertical splits without an automatic
     "New pane" choice. Their icons show a full-width bottom pane and a
     full-height right pane respectively.
231. **E-26-07** If I release the drag on a valid wheel choice, FileBlade runs the
     highlighted action once for all carried items.
232. **E-26-08** If I cancel the wheel or release without a valid choice, no file
     is opened, moved, or changed.
233. **E-26-09** If I release the drag on the hub of the wheel, the wheel stays
     open, so I can still pick with the pointer or a key instead of losing it.
234. **E-26-10** In the drop wheel, the Open with wedge shows an open-folder
     glyph whatever I am dragging, and its placements list the applications for
     those files.

**E-26-11** When I choose Review with hunk over a Herdr or tmux terminal,
I can choose New pane, New tab, New space, or New window. In tmux, a tab is
a tmux window and a space is a session; New window opens a separate terminal.
Each destination reviews the selected paths from their repository, or compares
two selected files. Over a plain terminal or empty desktop, Hunk opens directly
in a new terminal without offering multiplexer destinations.
Covered by `tests/vm/expectations/26-hunk-review.sh`.

**E-26-12** If the terminal I drop on shares one process with other windows
(ghostty, kitty in single-instance mode, foot in server mode, wezterm), the
wheel only offers pane, tab, space and paste placements when it can tell
which window I dropped on: for Herdr that is the window whose title names
exactly one workspace across my Herdr sessions, checked again when I pick a
placement. Otherwise the target reads "shared window", those placements are
missing, the same goes for "This nvim", and picking one anyway is refused
with the reason; New terminal and Review with hunk in a new window still
work. tmux gives no way to tell such windows apart, so it always counts as
shared there.

**E-26-13** If I set dragging out to hand files to the system, dragging rows
gives them to the application I drop on, so a browser upload field receives the
files themselves. That drag belongs to the compositor, so the drop wheel, its
keys and the drag scroll do not take part in it; dragging the row with my right
button keeps that gesture inside FileBlade instead and opens the wheel where I
release it outside the blade, without pressing the wheel key, and holding shift
or control as I press keeps it inside too, so the path pastes still work. With
the default setting every drag stays inside FileBlade, as it always has.

**E-26-14** I can drop files dragged from another application onto a folder row
in a blade, and a name containing a space arrives intact.

## 27. Updates and recovery

`tests/vm/expectations/27-updates-recovery.sh`

235. **E-27-01** If FileBlade or an enabled companion has an update available, I
     see an Update available chip in the blade footer.
This file was written by an agent.

236. **E-27-02** The update notice names the available FileBlade version, for
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
237. **E-27-03** If my FileBlade checkout has local work or commits that must
     not be overwritten, the update details tell me it was skipped.
238. **E-27-04** If FileBlade's interface and native helper are out of sync after
     an update, I see Backend update needed and instructions to update or reinstall it.
239. **E-27-05** If update checking is disabled or the network is unavailable,
     I can continue using FileBlade without repeated prompts or an endless busy
     indicator.
240. **E-27-06** If I check for updates and nothing is newer, I see "FileBlade is
     up to date!" in the footer and it disappears by itself after ten seconds.

## 28. Archives and long operations

`tests/vm/expectations/28-archives-operations.sh`

241. **E-28-01** If I select a supported archive, Extract here is available and
     places its contents in a clearly named destination beside the archive.
242. **E-28-02** If I extract an archive and its destination is already
     populated, FileBlade refuses, explains the conflict, and leaves the
     existing files unchanged.
243. **E-28-03** While a long file operation is running, I can see what FileBlade
     is doing and stop the operation from Properties.
244. **E-28-04** If I stop an operation, items already completed remain visible
     and any partial destination is refreshed immediately so I can inspect it.
245. **E-28-05** If an undo cannot continue safely, I see why; pressing `Shift+U`
     lets me skip that refused undo and continue to an older one.

## 29. Welcome and example extensions

`tests/vm/expectations/29-welcome.sh`

246. **E-29-01** On my first launch, the right blade holds a Notes tab and opens
     with a Welcome tab in front of it that asks "Install agent extensions?" and
     explains that FileBlade can be extended with example extensions for agent
     memory files, skills, MCPs, and hooks, or with my own extensions.
     Beneath Install, a yellow note explains that the extensions come directly
     from public GitHub repositories while awaiting a marketplace listing.
     Each extension name is a source link, usable by mouse or keyboard, that
     opens its GitHub repository without starting installation.
247. **E-29-02** If I click Install in the Welcome tab, the four example
     extensions are installed and enabled directly from GitHub at the exact
     versions this FileBlade was released with, with Omarchy's plugin checks
     and without a marketplace listing or terminal confirmation,
     the shell reloads once rather than once per extension, and the Welcome
     tab closes by itself. Installation keeps going through
     plugin reloads, with progress restored when FileBlade reappears. A pulsing
     FileBlade folder icon accompanies the installation count; disabling
     animations keeps the icon steady while preserving the progress text.
     Installation also works when my plugins directory is a symlink.
     When installation finishes, Skills, MCP, and Hooks appear together as a
     new section at the top of the right blade with Skills in front, and
     Memory joins the Notes section beneath it. The Welcome tab stays, showing
     "Adding tabs…", until those sections are saved, and it resumes adding them
     if FileBlade reloads in between. Extensions I had already placed stay
     where they are and are not added twice.
248. **E-29-03** If I click "Close and don't show this again", the Welcome tab
     closes and it does not come back on later launches or after a layout reset.
249. **E-29-04** After I install the extensions, the Welcome tab does not come
     back on later launches or after a layout reset, and extensions that are
     already installed are skipped rather than reinstalled.
250. **E-29-05** If an install fails, the Welcome tab stays open and shows which
     extension failed so I can retry. If a repository no longer offers the
     version this FileBlade was built against, nothing is installed from it and
     the tab says so. Retrying enables an extension left disabled
     by an interrupted install and continues with the remaining extensions. If
     the installed extensions cannot be added to the right blade within thirty
     seconds, the tab names the ones still missing and Install adds them again.

This file was written by an agent.

**E-29-06** If I enable Memory, Skills, MCP, Hooks, or a generated extension
before installing FileBlade, one pop-up lists the waiting extensions and shows
the FileBlade repository address. There is no Install button, and Enter does
nothing. Escape or the close button dismisses the pop-up. If FileBlade is already
installed but disabled, Enable activates that local installation and restarts the
shell. Once FileBlade is enabled, the pop-up disappears.

## 30. Configurable tree keys

`tests/vm/expectations/30-keybindings.sh`

251. **E-30-01** Editing `keybindings.json` changes tree navigation without a
     shell restart; overridden keys stop performing their old action and omitted
     actions keep their defaults.
252. **E-30-02** An empty binding array disables an action; key sequences can
     use a custom prefix and cancel on Escape or focus loss without opening,
     copying or deleting an item.
253. **E-30-03** Invalid or conflicting bindings preserve the last valid map and
     show an error. Fixing or removing the file clears the error and restores
     the corresponding bindings.
254. **E-30-04** Artifact trees inherit the same defaults and user overrides;
     the extension shortcut guide reports the effective bindings rather than a
     separately maintained keymap.

## 31. Scrolling stays put

`tests/vm/expectations/31-scrolling.sh`

255. **E-31-01** If I scroll the tree down and another application changes a
     file in a directory I am viewing, the rows I was looking at stay exactly
     where they were.
256. **E-31-02** If I scroll the tree down and press `Shift+R`, the tree
     refreshes and the same rows stay in view.
257. **E-31-03** A thin ruler on the right edge of the tree shows how far I have
     scrolled; it only appears when the tree is taller than the blade, and
     clicking near its top takes me back to the first rows.
258. **E-31-04** Files with a Git status leave a mark on the ruler at their
     position, coloured like their status, so I can see where changes are
     without scrolling.
259. **E-31-05** Marks for rows outside the part of the tree I can see are
     drawn at half strength; marks for rows in view are drawn in full. Turning
     off "Git marks on the scroll ruler" in settings removes every mark and the
     ruler thumb stays.
260. **E-31-06** Extension trees loaded through `ArtifactTree` show the same
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
- **E-36-04** Skills and Memory are browsable by default. Changes require explicitly
  enabling Manage agent files after its explanation; disabling it refuses further
  management actions, including CLI bin removal and restore.
- **E-36-05** Purging an MCP or Hooks removal deletes its private recovery too.
  Repeating removal and purge does not fill an invisible undo quota. Disabled
  companions must be explicitly enabled before restore or cleanup can run.
- **E-36-06** Welcome preserves an existing modified or differently pinned checkout
  and explains why it did not enable it. An update check downloads no Git objects
  and does not invent history details when those objects are not available.
