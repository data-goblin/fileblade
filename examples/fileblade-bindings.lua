local function fileblade(method, fallback)
  local call = "timeout 0.4s fileblade native ipc -- data-goblin.fileblade.control " .. method .. " >/dev/null 2>&1"
  if fallback then return call .. " || hyprctl dispatch " .. string.format("%q", fallback) end
  return call
end

hl.unbind("SUPER + B")
o.bind("SUPER + B", "Open or close the left blade", fileblade("toggleBladeFocus left"))
hl.unbind("SUPER + SHIFT + B")
o.bind("SUPER + SHIFT + B", "Open or close the right blade", fileblade("toggleBladeFocus right"))

for _, direction in ipairs({ { "LEFT", "l" }, { "RIGHT", "r" }, { "UP", "u" }, { "DOWN", "d" } }) do
  hl.unbind("SUPER + " .. direction[1])
  o.bind("SUPER + " .. direction[1], "Focus " .. direction[1]:lower() .. " (blade aware)", fileblade("focusDirection " .. direction[2], string.format('hl.dsp.focus({ direction = %q })', direction[2])))
  hl.unbind("SUPER + SHIFT + " .. direction[1])
  o.bind("SUPER + SHIFT + " .. direction[1], "Swap " .. direction[1]:lower() .. " (blade aware)", fileblade("windowSwap " .. direction[2], string.format('hl.dsp.window.swap({ direction = %q })', direction[2])))
end

hl.unbind("SUPER + W")
o.bind("SUPER + W", "Close window or blade", fileblade("windowClose", "hl.dsp.window.close()"))
hl.unbind("SUPER + T")
o.bind("SUPER + T", "Toggle window floating or blade dock", fileblade("windowToggle", 'hl.dsp.window.float({ action = "toggle" })'))

local resize_binds = {
  { "SUPER + code:20", "Expand window left", -100, 0 },
  { "SUPER + code:21", "Shrink window left", 100, 0 },
  { "SUPER + SHIFT + code:20", "Shrink window up", 0, -100 },
  { "SUPER + SHIFT + code:21", "Expand window down", 0, 100 },
  { "SUPER + ALT + code:20", "Expand window left a little", -25, 0 },
  { "SUPER + ALT + code:21", "Shrink window left a little", 25, 0 },
  { "SUPER + SHIFT + ALT + code:20", "Shrink window up a little", 0, -25 },
  { "SUPER + SHIFT + ALT + code:21", "Expand window down a little", 0, 25 },
  { "SUPER + CTRL + code:20", "Expand window left a lot", -300, 0 },
  { "SUPER + CTRL + code:21", "Shrink window left a lot", 300, 0 },
  { "SUPER + CTRL + SHIFT + code:20", "Shrink window up a lot", 0, -300 },
  { "SUPER + CTRL + SHIFT + code:21", "Expand window down a lot", 0, 300 },
}
for _, bind in ipairs(resize_binds) do
  hl.unbind(bind[1])
  o.bind(bind[1], bind[2] .. " (blade aware)", fileblade("windowResize " .. bind[3] .. " " .. bind[4], string.format("hl.dsp.window.resize({ x = %d, y = %d, relative = true })", bind[3], bind[4])))
end

-- Bound alongside Hyprland's own Super and right-button window resize, never
-- instead of it: these are non-consuming, so a drag over a window resizes the
-- window and the same drag over a docked blade resizes the blade
hl.bind("SUPER + mouse:273", hl.dsp.global("fileblade:resize-blade"), { non_consuming = true })
hl.bind("SUPER + mouse:273", hl.dsp.global("fileblade:resize-blade-end"), { release = true, non_consuming = true })
hl.bind("mouse:273", hl.dsp.global("fileblade:resize-blade-end"), { release = true, non_consuming = true })

o.bind("SUPER + Z", "Undo the last FileBlade file operation", fileblade("undo false false"))
