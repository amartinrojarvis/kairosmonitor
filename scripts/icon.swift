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
    let trace = NSBezierPath()
    trace.move(to: NSPoint(x: 192, y: 464))
    for (x, y) in [(350,464), (440,696), (566,304), (666,550), (832,550)] {
        trace.line(to: NSPoint(x: x, y: y))
    }
    trace.lineWidth = 60
    trace.lineCapStyle = .round
    trace.lineJoinStyle = .round
    NSColor(srgbRed: 0.27, green: 0.91, blue: 0.70, alpha: 1).setStroke()
    trace.stroke()
    NSGraphicsContext.restoreGraphicsState()
    let data = rep.representation(using: .png, properties: [:])!
    if size <= 512 { try data.write(to: URL(fileURLWithPath: "\(dest)/icon_\(size)x\(size).png")) }
    if size >= 32 { try data.write(to: URL(fileURLWithPath: "\(dest)/icon_\(size/2)x\(size/2)@2x.png")) }
}
