// Drive and capture only the NSSavePanel owned by the supplied acceptance PID.
import AppKit
import ApplicationServices
import CoreGraphics

func attr(_ element: AXUIElement, _ name: String) -> AnyObject? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}
func find(_ element: AXUIElement, role: String, title: String, depth: Int = 0) -> AXUIElement? {
    if attr(element, kAXRoleAttribute) as? String == role && (title.isEmpty || attr(element, kAXTitleAttribute) as? String == title) { return element }
    if depth < 15, let children = attr(element, kAXChildrenAttribute) as? [AXUIElement] {
        for child in children { if let match = find(child, role: role, title: title, depth: depth + 1) { return match } }
    }
    return nil
}
func key(_ pid: pid_t, _ code: CGKeyCode, _ flags: CGEventFlags = []) {
    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else { fatalError("Owned panel lost keyboard focus") }
    for down in [true,false] { let event=CGEvent(keyboardEventSource:nil, virtualKey:code, keyDown:down)!; event.flags=flags; event.post(tap:.cghidEventTap) }
}
func press(_ button: AXUIElement) throws {
    guard AXUIElementPerformAction(button, kAXPressAction as CFString) == .success else { throw NSError(domain:"panel", code:2) }
}
let args=CommandLine.arguments
let pid=pid_t(args[1])!
let app=AXUIElementCreateApplication(pid)
NSRunningApplication(processIdentifier:pid)?.activate(options:[])
var panel: AXUIElement?
for _ in 0..<100 {
    if let windows=attr(app,kAXWindowsAttribute) as? [AXUIElement] {
        panel=windows.first { find($0,role:kAXButtonRole,title:"Cancel") != nil && find($0,role:kAXButtonRole,title:"Save") != nil }
    }
    if panel != nil { break }
    Thread.sleep(forTimeInterval:0.1)
}
guard let panel=panel, let cancel=find(panel,role:kAXButtonRole,title:"Cancel") else { fatalError("Owned system Save panel unavailable") }
let windows=CGWindowListCopyWindowInfo(.optionOnScreenOnly,kCGNullWindowID) as? [[String:Any]] ?? []
guard let window=windows.first(where:{ ($0[kCGWindowOwnerPID as String] as? Int)==Int(pid) && ($0[kCGWindowName as String] as? String)=="Save" }),
      let id=window[kCGWindowNumber as String] as? Int else { fatalError("Owned system Save window cannot be captured") }
let screenshot=Process(); screenshot.executableURL=URL(fileURLWithPath:"/usr/sbin/screencapture"); screenshot.arguments=["-x","-l",String(id),args[2]]
try screenshot.run(); screenshot.waitUntilExit(); guard screenshot.terminationStatus==0 else { fatalError("Owned panel capture failed") }
if args.count==3 { try press(cancel) } else {
    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else { fatalError("Owned panel lost keyboard focus") }
    let shortcut=Process(); shortcut.executableURL=URL(fileURLWithPath:"/usr/bin/osascript")
    shortcut.arguments=["-e", "tell application \"System Events\" to keystroke \"g\" using {command down, shift down}"]
    try shortcut.run(); shortcut.waitUntilExit(); Thread.sleep(forTimeInterval:0.7)
    var focusedValue: CFTypeRef?
    guard AXUIElementCopyAttributeValue(app,kAXFocusedUIElementAttribute as CFString,&focusedValue) == .success, let focusedValue=focusedValue else { fatalError("Go to Folder field unavailable") }
    let focused=focusedValue as! AXUIElement
    guard find(panel,role:"AXSheet",title:"") != nil, attr(focused,kAXRoleAttribute) as? String == kAXTextFieldRole else { fatalError("Go to Folder did not open") }
    guard AXUIElementSetAttributeValue(focused,kAXValueAttribute as CFString,args[3] as CFString) == .success else { fatalError("Cannot enter destination") }
    key(pid,36); Thread.sleep(forTimeInterval:0.7)
    guard let save=find(panel,role:kAXButtonRole,title:"Save") else { fatalError("Owned panel Save button unavailable") }
    try press(save)
}
print("done")
