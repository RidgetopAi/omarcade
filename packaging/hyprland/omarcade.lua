-- Omarcade window rules for Hyprland (Omarchy 4 "Quattro" Lua config).
--
-- Install: copy this file to ~/.config/hypr/omarcade.lua, then add
--
--     require("hypr.omarcade")
--
-- to the bottom of ~/.config/hypr/hyprland.lua.
--
-- See https://wiki.hypr.land/Configuring/Basics/Window-Rules/

-- Every Omarcade game reports the same Wayland app_id, so one rule set
-- governs the whole suite. Anchored: a future "omarcade-launcher" must
-- opt in deliberately rather than inherit these by accident.
local OMARCADE = "^(omarcade)$"

-- Float and center. The games render a fixed 4:3 canvas and letterbox
-- anything else, so tiling them into an arbitrary rectangle just grows
-- the black bars. 960x720 is the games' native size (see WIDTH/HEIGHT).
o.window(OMARCADE, { float = true })
o.window(OMARCADE, { center = true })
o.window(OMARCADE, { size = { 960, 720 } })

-- Opt out of Omarchy's default translucency. The default is
-- opacity 0.985 0.96, which desaturates the palette the games read
-- from the active theme -- and the effect compounds with our own
-- letterbox. RetroArch and Steam opt out the same way.
o.window(OMARCADE, { tag = "-default-opacity", opacity = "1 1" })

-- Don't blank the screen mid-game. Gameplay is keyboard-only, so
-- Hyprland's idle timer sees no pointer motion and would otherwise
-- dim or lock during a long rally.
o.window(OMARCADE, { idle_inhibit = "always" })
