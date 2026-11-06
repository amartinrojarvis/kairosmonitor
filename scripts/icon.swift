import AppKit

// Native vector artwork, rasterized at build time. No runtime assets or image libraries.
let dest = CommandLine.arguments[1]
try FileManager.default.createDirectory(atPath: dest, withIntermediateDirectories: true)
for size in [16, 32, 64, 128, 256, 512, 1024] {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let scale = CGFloat(size) / 1024
    let transform = AffineTransform(scale: scale)
    (transform as NSAffineTransform).concat()
    let shape = NSBezierPath(roundedRect: NSRect(x: 32, y: 32, width: 960, height: 960), xRadius: 220, yRadius: 220)
    NSColor(srgbRed: 0.08, green: 0.12, blue: 0.16, alpha: 1).setFill()
    shape.fill()
    // Kairos: a precise K, with a quiet clock arc suggesting time and monitoring.
    let rim = NSBezierPath(roundedRect: NSRect(x: 50, y: 50, width: 924, height: 924), xRadius: 204, yRadius: 204)
    rim.lineWidth = 3
    NSColor(srgbRed: 0.25, green: 0.32, blue: 0.38, alpha: 0.65).setStroke()
    rim.stroke()
    let clock = NSBezierPath()
    clock.appendArc(withCenter: NSPoint(x: 512, y: 512), radius: 350, startAngle: 35, endAngle: 325, clockwise: false)
    clock.lineWidth = 22
    clock.lineCapStyle = .round
    NSColor(srgbRed: 0.44, green: 0.64, blue: 0.61, alpha: 0.38).setStroke()
    clock.stroke()
    let stem = NSBezierPath(roundedRect: NSRect(x: 306, y: 280, width: 78, height: 464), xRadius: 39, yRadius: 39)
    NSColor(srgbRed: 0.63, green: 0.79, blue: 0.74, alpha: 1).setFill()
    stem.fill()
    let arms = NSBezierPath()
    arms.move(to: NSPoint(x: 666, y: 712))
    arms.line(to: NSPoint(x: 440, y: 512))
    arms.line(to: NSPoint(x: 674, y: 304))
    arms.lineWidth = 76
    arms.lineCapStyle = .round
    arms.lineJoinStyle = .round
    NSColor(srgbRed: 0.88, green: 0.92, blue: 0.92, alpha: 1).setStroke()
    arms.stroke()
    let indicator = NSBezierPath(ovalIn: NSRect(x: 792, y: 682, width: 46, height: 46))
    NSColor(srgbRed: 0.63, green: 0.79, blue: 0.74, alpha: 1).setFill()
    indicator.fill()
    NSGraphicsContext.restoreGraphicsState()
    let data = rep.representation(using: .png, properties: [:])!
    if size <= 512 { try data.write(to: URL(fileURLWithPath: "\(dest)/icon_\(size)x\(size).png")) }
    if size >= 32 { try data.write(to: URL(fileURLWithPath: "\(dest)/icon_\(size/2)x\(size/2)@2x.png")) }
}
