#NoEnv
#SingleInstance Force
#Persistent
#InstallKeybdHook
#InstallMouseHook
#MaxThreadsPerHotkey 1
#MaxThreadsBuffer Off

SetBatchLines, -1
SetMouseDelay, -1
SendMode Input
CoordMode, Pixel, Screen
CoordMode, Mouse, Screen

global SCAN_RADIUS := 12
global SCAN_INTERVAL := 5
global REARM_DELAY := 120
global scanning := false
global busy := false
global lastClick := 0
OnExit, ScriptExit

~*$e::
    if (scanning)
        return
    scanning := true
    SetTimer, CheckGreen, %SCAN_INTERVAL%
return

~*$e up::
    scanning := false
    SetTimer, CheckGreen, Off
return

CheckGreen:
    Critical, On
    if (busy) {
        Critical, Off
        return
    }
    if (!scanning || !GetKeyState("e", "P")) {
        scanning := false
        SetTimer, CheckGreen, Off
        Critical, Off
        return
    }
    if (lastClick && A_TickCount - lastClick < REARM_DELAY) {
        Critical, Off
        return
    }
    busy := true
    Critical, Off
    MouseGetPos, mx, my
    greenFound := false
    radius := SCAN_RADIUS
    diameter := radius * 2 + 1
    Loop, %diameter% {
        offsetY := A_Index - radius - 1
        Loop, %diameter% {
            offsetX := A_Index - radius - 1
            x := mx + offsetX
            y := my + offsetY
            PixelGetColor, color, %x%, %y%, RGB
            if (ErrorLevel)
                continue
            red := (color >> 16) & 255
            green := (color >> 8) & 255
            blue := color & 255
            isGreen := (green >= 75
                && green > red * 1.35
                && green > blue * 1.35
                && green - red >= 35
                && green - blue >= 35)
            if (isGreen) {
                greenFound := true
                break
            }
        }
        if (greenFound)
            break
    }
    Critical, On
    if (greenFound
        && scanning
        && GetKeyState("e", "P")
        && (!lastClick || A_TickCount - lastClick >= REARM_DELAY)) {
        lastClick := A_TickCount
        Click
    }
    busy := false
    Critical, Off
return

ScriptExit:
    FileAppend, %A_Now% - Script exited`n, %A_ScriptDir%\triggerbot_exit_log.txt
    ExitApp
return

F8::ExitApp
