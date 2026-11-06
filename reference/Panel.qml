import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

// Kairos Dash bar widget + popup.
//
// The bar pill shows three numbers; clicking it opens a panel that lays the
// same telemetry out the way Kairos Dash does — "$"-prefixed uppercase labels,
// big tabular values, thin meters, square status dot, dot-graph — but painted
// entirely from the active Omarchy theme (popup chrome, spacing and font
// tokens, and the theme's own green/yellow/red for the ok/warn/danger tones).
Panel {
  id: root

  moduleName: "amartin.kairos"
  ipcTarget: "amartin.kairos"

  // ---------------------------------------------------------------- settings
  readonly property string metricsUrl: String(root.setting("url", "http://kairos:8787/api/metrics"))
  readonly property string dashboardUrl: String(root.setting("dashboard", "http://kairos:8787/"))
  readonly property string serviceLabel: String(root.setting("service", "vllmcritic"))
  readonly property int pollSeconds: Math.max(1, Math.round(Number(root.setting("interval", 5)) || 5))
  readonly property int openPollSeconds: Math.max(1, Math.round(Number(root.setting("openInterval", 2)) || 2))
  readonly property int timeoutSeconds: Math.max(1, Math.round(Number(root.setting("timeout", 4)) || 4))
  readonly property real memWarn: Number(root.setting("memWarn", 85))
  readonly property real memDanger: Number(root.setting("memDanger", 95))
  readonly property real gpuWarn: Number(root.setting("gpuWarn", 95))
  readonly property real gpuDanger: Number(root.setting("gpuDanger", 101))
  readonly property real tempWarn: Number(root.setting("tempWarn", 78))
  readonly property real tempDanger: Number(root.setting("tempDanger", 88))
  readonly property real fillAlpha: Number(root.setting("fillAlpha", 0.10))
  readonly property real borderAlpha: Number(root.setting("borderAlpha", 0.16))
  readonly property bool showBorder: root.setting("border", true) !== false
  readonly property bool showGraph: root.setting("graph", true) !== false
  readonly property int historyLength: Math.max(8, Math.round(Number(root.setting("history", 44)) || 44))
  // Digits reserved for each bar value; long enough for "100%", "104G", "1.2K".
  readonly property int valueChars: Math.max(2, Math.round(Number(root.setting("valueChars", 4)) || 4))

  // ------------------------------------------------------------------- state
  property var sample: null
  property bool online: false
  property bool fetchedOnce: false
  property string lastError: ""
  property real lastOkAt: 0
  property real lastSampleAt: 0
  property var tpsHistory: []
  property var themeColors: ({})

  // The bar geometry helpers live on BarWidget, which this entry point is
  // not — Panel only injects bar/moduleName/settings.
  readonly property bool vertical: bar ? bar.vertical : false
  readonly property int barSize: bar ? bar.barSize : Style.bar.sizeHorizontal
  readonly property color ink: bar ? bar.barForeground : Color.foreground
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  // One glyph of the bar's monospace font, and the width a chip needs for the
  // icon, a space and a padded value. Chips get this as fixedWidth so the pill
  // — and everything to its right in the bar — never moves when a number
  // changes from "9%" to "92%".
  readonly property real glyphWidth: chipMetrics.advanceWidth / 4
  readonly property real chipWidth: Math.round(root.glyphWidth * (root.valueChars + 2)
    + Style.spaceReal(6) * 2)
  readonly property color chipFill: Util.alpha(ink, root.fillAlpha)
  readonly property color chipBorder: Util.alpha(ink, root.borderAlpha)
  readonly property real cardRadius: Math.min(Style.cornerRadius, 3)

  // ------------------------------------------------------- theme-derived ink
  // colors.toml disagrees between themes (tahoe-dark ships `green`, ANSI-style
  // themes ship `color2`/`color10`), so every tone has candidate keys and a
  // fallback into the foundational palette.
  readonly property bool themeDark: String(themeColors.mode || "dark") === "dark"
  readonly property color okColor: root.tune(Model.themeColor(themeColors, ["green", "color2", "color10"], Color.accent))
  readonly property color warnColor: root.tune(Model.themeColor(themeColors, ["yellow", "color3", "color11"], Color.urgent))
  readonly property color dangerColor: root.tune(Model.themeColor(themeColors, ["red", "color1", "color9"], Color.urgent))
  readonly property color dimColor: Model.themeColor(themeColors, ["muted", "dark_foreground"], Color.muted)

  function tune(value) {
    var c = Qt.color(value)
    return root.themeDark ? Qt.lighter(c, 1.28) : Qt.darker(c, 1.12)
  }

  function stateColor(state) {
    if (state === "danger") return root.dangerColor
    if (state === "warn") return root.warnColor
    return root.okColor
  }

  // ------------------------------------------------------------- bar labels
  readonly property string memValue: root.online && root.sample && root.sample.memUsed > 0
    ? Model.formatGiBShort(root.sample.memUsed) : "--"
  readonly property string memGibValue: root.online && root.sample && root.sample.memUsed > 0
    ? Model.formatGiB(root.sample.memUsed) : "--"
  readonly property string gpuValue: root.online && root.sample
    ? Model.formatPercent(root.sample.gpuPercent) : "--"
  readonly property string tpsValue: root.online && root.sample
    ? Model.formatTps(root.sample.tps) : "--"

  readonly property string memState: root.online && root.sample
    ? Model.stateOf(root.sample.memPercent, root.memWarn, root.memDanger) : "none"
  readonly property string gpuState: root.online && root.sample
    ? Model.stateOf(root.sample.gpuTemp, root.tempWarn, root.tempDanger) : "none"
  readonly property string tempState: root.online && root.sample
    ? Model.stateOf(root.sample.tempMax, root.tempWarn, root.tempDanger) : "none"

  // ------------------------------------------------------------- panel data
  readonly property var service: root.sample ? Model.serviceFor(root.sample, root.serviceLabel) : null
  readonly property bool serviceRunning: root.service ? root.service.running === true : false
  readonly property string panelModel: root.sample && root.sample.model
    ? String(root.sample.model)
    : (root.service && root.service.model ? String(root.service.model) : "—")
  readonly property string panelContext: root.service && root.num(root.service.maxModelLen) > 0
    ? String(Math.round(root.num(root.service.maxModelLen) / 1000)) + "K ctx" : ""
  readonly property string consumerText: {
    var labels = Model.consumerLabels(root.sample)
    return labels.length > 0 ? labels.join("  ") : "sin peticiones activas"
  }
  readonly property string metaLine: {
    if (!root.online || !root.sample) return root.lastError !== "" ? root.lastError : "esperando lectura…"
    var bits = []
    if (root.sample.uptime >= 0) bits.push("uptime " + Model.formatUptime(root.sample.uptime))
    bits.push("actualizado " + Model.formatClock(root.lastSampleAt))
    if (root.sample.tailscale !== "") bits.push("tailscale " + root.sample.tailscale)
    return bits.join("  ·  ")
  }
  readonly property string peakLabel: root.online && root.sample && root.sample.maxTps >= 0
    ? "pico " + Model.formatTps(root.sample.maxTps) : "pico —"

  // Right-aligned window of the tok/s history, padded to a fixed slot count.
  // The samples actually drawn: the last N readings, no empty padding. The
  // graph gives every sample the same slice of the frame, so it always spans
  // the full width instead of creeping in from the middle as it fills up.
  readonly property var tpsSeries: {
    var history = Array.isArray(root.tpsHistory) ? root.tpsHistory : []
    var start = Math.max(0, history.length - root.historyLength)
    var out = []
    for (var i = start; i < history.length; i++) out.push(num(history[i]))
    return out
  }
  readonly property real tpsPeak: {
    var peak = 1
    var series = root.tpsSeries
    for (var i = 0; i < series.length; i++) if (num(series[i]) > peak) peak = num(series[i])
    return peak
  }

  function num(value) {
    var n = Number(value)
    return isFinite(n) ? n : 0
  }

  // Pad a bar value to the reserved number of glyphs. The font is monospace, so
  // trailing spaces are exactly as wide as digits.
  function pad(value) {
    var text = String(value)
    while (text.length < root.valueChars) text += " "
    return text
  }

  function refresh() {
    // A dashboard that stops answering must not keep showing the last good
    // numbers forever.
    if (root.fetchedOnce && root.online
        && root.lastOkAt > 0 && Date.now() - root.lastOkAt > root.pollSeconds * 3000 + 8000) {
      root.online = false
      root.lastError = "sin respuesta"
    }
    if (!fetchProc.running) fetchProc.running = true
  }

  function applyPayload(raw) {
    var parsed = Model.parseMetrics(raw)
    root.fetchedOnce = true
    if (!parsed.ok) {
      root.online = false
      root.lastError = parsed.error
      return
    }
    root.sample = parsed
    root.online = true
    root.lastError = ""
    root.lastOkAt = Date.now()
    root.lastSampleAt = Date.now()
    if (parsed.tps >= 0) {
      // Always hand the property a fresh array: reassigning the same JS array
      // is not seen as a change, and the graph would never redraw.
      var history = Array.isArray(root.tpsHistory) ? root.tpsHistory.slice() : []
      history.push(parsed.tps)
      var cap = root.historyLength * 2
      if (history.length > cap) history = history.slice(history.length - cap)
      root.tpsHistory = history
    }
  }

  function openDashboard() {
    if (root.bar) root.bar.run("xdg-open " + Util.shellQuote(root.dashboardUrl))
  }

  function open() {
    root.controller.show()
    root.refresh()
  }

  function handlePress(button) {
    if (button === Qt.MiddleButton) root.refresh()
    else if (button === Qt.RightButton) root.openDashboard()
    else root.toggle()
  }

  // ------------------------------------------------------------- data source
  Process {
    id: fetchProc
    command: ["curl", "-sS", "--max-time", String(root.timeoutSeconds), root.metricsUrl]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.applyPayload(text)
    }
    onExited: function(code) {
      if (code !== 0) {
        root.online = false
        root.fetchedOnce = true
        root.lastError = "kairos no responde"
      }
    }
  }

  // Theme palette, so the panel's ok/warn/danger tones are the theme's own.
  property FileView colorsFile: FileView {
    path: Color.currentThemePath + "/colors.toml"
    watchChanges: true
    printErrors: false
    onLoaded: root.themeColors = Model.parseColors(text())
    onFileChanged: reload()
    onLoadFailed: root.themeColors = ({})
  }

  Timer {
    interval: root.opened ? root.openPollSeconds * 1000 : root.pollSeconds * 1000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  // ------------------------------------------------------------- bar widgets
  TextMetrics {
    id: chipMetrics
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
    text: "0000"
  }

  component KairosChip: WidgetButton {
    id: chip

    property string icon: ""
    property string value: ""
    property string tip: ""
    property color tone: root.ink
    property bool toneOverride: false

    bar: root.bar
    text: root.vertical ? chip.icon : chip.icon + " " + chip.value
    fixedWidth: root.vertical ? -1 : root.chipWidth
    tooltipText: chip.tip
    active: chip.toneOverride
    activeColor: chip.tone
    dimmed: !root.online
    horizontalMargin: 6
    fontSize: Style.font.bodySmall
    onPressed: function(button) { root.handlePress(button) }
  }

  Rectangle {
    id: pill
    visible: !root.vertical
    anchors.verticalCenter: parent.verticalCenter
    anchors.horizontalCenter: parent.horizontalCenter
    width: chipRow.implicitWidth + Style.spaceReal(14)
    height: Math.max(12, root.barSize - Style.space(6))
    radius: height / 2
    color: root.chipFill
    border.width: root.showBorder ? 1 : 0
    border.color: root.chipBorder
    opacity: root.online ? 1.0 : 0.75

    // The gaps between chips belong to the pill, not to any one chip.
    MouseArea {
      anchors.fill: parent
      acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: function(mouse) { root.handlePress(mouse.button) }
    }

    Row {
      id: chipRow
      anchors.centerIn: parent
      height: root.barSize
      spacing: Style.space(6)

      KairosChip {
        icon: Model.ICON_GPU
        value: root.pad(root.gpuValue)
        tip: Model.gpuTooltip(root.sample)
        toneOverride: root.gpuState === "danger"
        tone: root.stateColor(root.gpuState)
      }

      KairosChip {
        icon: Model.ICON_MEMORY
        value: root.pad(root.memValue)
        tip: Model.memTooltip(root.sample)
        toneOverride: root.memState !== "ok" && root.memState !== "none"
        tone: root.stateColor(root.memState)
      }

      KairosChip {
        icon: Model.ICON_TPS
        value: root.pad(root.tpsValue)
        tip: Model.tpsTooltip(root.sample, root.serviceLabel)
      }
    }
  }

  Rectangle {
    id: pillVertical
    visible: root.vertical
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.verticalCenter: parent.verticalCenter
    width: Math.max(12, root.barSize - Style.space(6))
    height: chipColumn.implicitHeight + Style.spaceReal(12)
    radius: width / 2
    color: root.chipFill
    border.width: root.showBorder ? 1 : 0
    border.color: root.chipBorder
    opacity: root.online ? 1.0 : 0.75

    Column {
      id: chipColumn
      anchors.centerIn: parent
      spacing: Style.space(2)

      KairosChip {
        icon: Model.ICON_GPU
        value: root.pad(root.gpuValue)
        tip: Model.gpuTooltip(root.sample)
        toneOverride: root.gpuState === "danger"
        tone: root.stateColor(root.gpuState)
      }

      KairosChip {
        icon: Model.ICON_MEMORY
        value: root.pad(root.memValue)
        tip: Model.memTooltip(root.sample)
        toneOverride: root.memState !== "ok" && root.memState !== "none"
        tone: root.stateColor(root.memState)
      }

      KairosChip {
        icon: Model.ICON_TPS
        value: root.pad(root.tpsValue)
        tip: Model.tpsTooltip(root.sample, root.serviceLabel)
      }
    }
  }

  implicitWidth: root.vertical ? root.barSize : pill.width + Style.space(6)
  implicitHeight: root.vertical ? pillVertical.height : root.barSize

  // ------------------------------------------------------------------ pieces
  // "$ LABEL" — the dashboard's terminal-prompt section heading.
  component PanelLabel: Row {
    id: panelLabel

    property string label: ""
    property real size: Style.font.caption
    property color tone: root.dimColor

    spacing: Style.space(4)

    Text {
      text: "$"
      color: root.okColor
      font.family: root.fontFamily
      font.pixelSize: panelLabel.size
      font.bold: true
    }

    Text {
      textFormat: Text.PlainText
      text: panelLabel.label.toUpperCase()
      color: panelLabel.tone
      font.family: root.fontFamily
      font.pixelSize: panelLabel.size
      font.letterSpacing: Style.spaceReal(1)
    }
  }

  component Separator: Rectangle {
    height: 1
    color: Util.alpha(root.ink, 0.10)
  }

  component KairosMeter: Rectangle {
    id: meter

    property real fraction: 0
    property color tone: root.okColor

    height: Style.space(7)
    radius: root.cardRadius
    color: Util.alpha(root.ink, 0.05)
    border.width: 1
    border.color: Util.alpha(root.ink, 0.12)

    Rectangle {
      height: parent.height
      width: Math.max(0, Math.min(1, meter.fraction)) * parent.width
      radius: root.cardRadius
      color: meter.tone
      opacity: 0.9

      Behavior on width {
        NumberAnimation { duration: 220; easing.type: Easing.OutCubic }
      }
      Behavior on color {
        ColorAnimation { duration: 220 }
      }
    }
  }

  // One dashboard card: label, big value, meter, footnote.
  component MetricCard: Rectangle {
    id: card

    property string label: ""
    property string value: "--"
    property real fraction: 0
    property string detail: ""
    property string detail2: ""
    property color tone: root.okColor
    property bool muted: !root.online

    radius: root.cardRadius
    color: Util.alpha(root.ink, 0.045)
    border.width: 1
    border.color: Util.alpha(root.ink, 0.13)
    implicitHeight: cardColumn.implicitHeight + Style.spaceReal(24)

    Column {
      id: cardColumn
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: Style.space(12)
      spacing: Style.space(8)

      PanelLabel {
        label: card.label
        tone: card.muted ? Util.alpha(root.dimColor, 0.7) : root.dimColor
      }

      Text {
        textFormat: Text.PlainText
        text: card.value
        color: card.muted ? Util.alpha(root.dimColor, 0.7) : card.tone
        font.family: root.fontFamily
        font.pixelSize: Style.font.display
        font.weight: Font.Medium
      }

      KairosMeter {
        width: cardColumn.width
        fraction: card.muted ? 0 : card.fraction
        tone: card.tone
      }

      Text {
        textFormat: Text.PlainText
        visible: card.detail !== ""
        width: cardColumn.width
        text: card.detail
        color: Util.alpha(root.dimColor, 0.95)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        lineHeight: 1.2
        wrapMode: Text.WordWrap
      }

      Text {
        textFormat: Text.PlainText
        visible: card.detail2 !== ""
        width: cardColumn.width
        text: card.detail2
        color: Util.alpha(root.dimColor, 0.7)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
    }
  }

  // Small key/value in the secondary telemetry strip.
  component StatCell: Column {
    id: cell

    property string label: ""
    property string value: "--"
    property color tone: root.ink

    spacing: Style.space(3)

    Text {
      textFormat: Text.PlainText
      text: cell.label.toUpperCase()
      color: Util.alpha(root.dimColor, 0.85)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.letterSpacing: Style.spaceReal(1)
      width: cell.width
      elide: Text.ElideRight
    }

    Text {
      textFormat: Text.PlainText
      text: cell.value
      color: cell.tone
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      width: cell.width
      elide: Text.ElideMiddle
    }
  }

  // The dashboard's dot-matrix graph, one column per sample. Columns share the
  // frame width, so the drawing always reaches both edges.
  component DotGraph: Row {
    id: graph

    property var slots: []
    property real peak: 1
    property color tone: root.okColor

    readonly property int columns: Math.max(1, graph.slots.length)
    readonly property real columnWidth: (width - spacing * (columns - 1)) / columns

    spacing: Style.space(2)

    Repeater {
      model: graph.slots

      Rectangle {
        required property var modelData

        width: graph.columnWidth
        height: Math.max(Style.space(2),
          modelData >= 0 ? Math.min(1, root.num(modelData) / graph.peak) * graph.height : 0)
        // Rows stack along x only; y keeps the bars bottom-aligned.
        y: graph.height - height
        radius: 1
        color: Util.alpha(graph.tone,
          0.35 + 0.6 * Math.min(1, root.num(modelData) / graph.peak))
      }
    }
  }

  // ------------------------------------------------------------------- panel
  KeyboardPanel {
    id: panel
    anchorItem: root.vertical ? pillVertical : pill
    owner: root
    bar: root.bar
    open: root.opened
    contentWidth: panel.fittedContentWidth(Style.space(620))
    contentHeight: panel.fittedContentHeight(dashColumn.implicitHeight, Style.space(760))

    Column {
      id: dashColumn
      width: parent.width
      spacing: Style.space(12)

      // header — eyebrow + host on the left, status pill on the right
      Item {
        width: parent.width
        implicitHeight: Math.max(headerLeft.implicitHeight, statusPill.height)

        Column {
          id: headerLeft
          anchors.left: parent.left
          anchors.right: statusPill.left
          anchors.rightMargin: Style.space(12)
          anchors.verticalCenter: parent.verticalCenter
          spacing: Style.space(4)

          PanelLabel {
            label: "kairos dash"
            size: Style.font.caption
            tone: root.okColor
          }

          Text {
            textFormat: Text.PlainText
            text: (root.online && root.sample && root.sample.hostname !== ""
              ? root.sample.hostname : root.serviceLabel).toUpperCase()
            color: root.ink
            font.family: root.fontFamily
            font.pixelSize: Style.font.title
            font.letterSpacing: Style.spaceReal(1)
            elide: Text.ElideRight
            width: parent.width
          }

          Text {
            textFormat: Text.PlainText
            text: root.metaLine
            color: Util.alpha(root.dimColor, 0.95)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            elide: Text.ElideMiddle
            width: parent.width
          }
        }

        Rectangle {
          id: statusPill
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          width: statusRow.width + Style.spaceReal(16)
          height: statusRow.height + Style.spaceReal(12)
          radius: root.cardRadius
          color: Util.alpha(root.ink, 0.05)
          border.width: 1
          border.color: Util.alpha(root.online ? root.okColor : root.dangerColor, 0.45)

          Row {
            id: statusRow
            anchors.centerIn: parent
            spacing: Style.space(6)

            Rectangle {
              anchors.verticalCenter: parent.verticalCenter
              width: Style.space(9)
              height: Style.space(9)
              radius: 1
              color: root.online ? root.okColor : root.dangerColor
            }

            Text {
              textFormat: Text.PlainText
              text: root.online ? "online" : "offline"
              color: root.online ? root.okColor : root.dangerColor
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              font.letterSpacing: Style.spaceReal(1)
            }
          }
        }
      }

      Separator { width: parent.width }

      Row {
        id: metricsRow
        width: parent.width
        spacing: Style.space(10)
        // Cards share the tallest content height. Taking it from the cards'
        // implicitHeight (never from Row.height) keeps the row positioner out
        // of a height <-> implicitHeight dependency loop, which Qt resolves
        // by collapsing everything to 0.
        readonly property real cardHeight: Math.max(
          gpuCard.implicitHeight, memCard.implicitHeight, tpsCard.implicitHeight)
        height: cardHeight

        MetricCard {
          id: gpuCard
          width: (metricsRow.width - metricsRow.spacing * 2) / 3
          height: metricsRow.cardHeight
          label: "uso de gpu"
          value: root.gpuValue
          fraction: root.online && root.sample ? root.sample.gpuPercent / 100 : 0
          tone: root.stateColor(root.gpuState)
          detail: root.online && root.sample
            ? ((root.sample.gpuName !== "" ? root.sample.gpuName : "GPU")
               + (root.sample.gpuTemp >= 0 ? "  ·  " + Math.round(root.sample.gpuTemp) + "°C" : ""))
            : "sin datos"
          detail2: root.online && root.sample && root.sample.gpuPower >= 0
            ? (Math.round(root.sample.gpuPower * 10) / 10) + " W" : ""
        }

        MetricCard {
          id: memCard
          width: (metricsRow.width - metricsRow.spacing * 2) / 3
          height: metricsRow.cardHeight
          label: "system memory"
          value: root.memGibValue
          fraction: root.online && root.sample && root.sample.memTotal > 0
            ? root.sample.memPercent / 100 : 0
          tone: root.stateColor(root.memState)
          detail: root.online && root.sample && root.sample.memTotal > 0
            ? "de " + Model.formatGiB(root.sample.memTotal)
              + "  ·  " + Model.formatPercent(root.sample.memPercent)
            : "sin datos"
          detail2: root.online && root.sample && root.sample.swapPercent >= 0
            ? "swap " + Model.formatPercent(root.sample.swapPercent) : ""
        }

        MetricCard {
          id: tpsCard
          width: (metricsRow.width - metricsRow.spacing * 2) / 3
          height: metricsRow.cardHeight
          label: root.serviceLabel + " tps"
          value: root.tpsValue
          fraction: root.online && root.sample && root.sample.maxTps > 0
            ? root.sample.tps / root.sample.maxTps : 0
          tone: root.online && root.sample && root.sample.tps >= 0 ? root.okColor : root.dimColor
          detail: root.online && root.sample
            ? Math.round(root.sample.running) + (root.sample.running === 1 ? " activa" : " activas")
              + (root.sample.waiting > 0 ? "  ·  " + Math.round(root.sample.waiting) + " en cola" : "")
              + "  ·  " + root.peakLabel
            : "sin tráfico"
          detail2: root.service ? (root.serviceRunning ? "servicio online" : "servicio detenido") : ""
        }
      }

      Separator { width: parent.width }

      // secondary telemetry strip
      Grid {
        id: statsGrid
        width: parent.width
        columns: 4
        columnSpacing: Style.space(10)
        rowSpacing: Style.space(10)

        StatCell {
          width: (statsGrid.width - statsGrid.columnSpacing * 3) / 4
          label: "cpu"
          value: root.online && root.sample ? Model.formatPercent(root.sample.cpuPercent) : "--"
          tone: root.ink
        }

        StatCell {
          width: (statsGrid.width - statsGrid.columnSpacing * 3) / 4
          label: "load"
          value: root.online && root.sample && root.sample.load1 >= 0
            ? root.sample.load1.toFixed(2) + " / " + root.sample.cpuCores : "--"
          tone: root.ink
        }

        StatCell {
          width: (statsGrid.width - statsGrid.columnSpacing * 3) / 4
          label: "net ↓ ↑"
          value: root.online && root.sample
            ? Model.formatRate(root.sample.netRx) + "  " + Model.formatRate(root.sample.netTx) : "--"
          tone: root.ink
        }

        StatCell {
          width: (statsGrid.width - statsGrid.columnSpacing * 3) / 4
          label: "temp"
          value: root.online && root.sample && root.sample.tempMax >= 0
            ? Math.round(root.sample.tempMax) + "°C" : "--"
          tone: root.stateColor(root.tempState)
        }

        StatCell {
          width: (statsGrid.width - statsGrid.columnSpacing * 3) / 4
          label: "disco /"
          value: root.online && root.sample && root.sample.diskPercent >= 0
            ? Model.formatPercent(root.sample.diskPercent) : "--"
          tone: root.ink
        }

        StatCell {
          width: (statsGrid.width - statsGrid.columnSpacing * 3) / 4
          label: "modelo"
          value: root.panelModel
          tone: root.ink
        }

        StatCell {
          width: (statsGrid.width - statsGrid.columnSpacing * 3) / 4
          label: "contexto"
          value: root.panelContext !== "" ? root.panelContext : "--"
          tone: root.ink
        }

        StatCell {
          width: (statsGrid.width - statsGrid.columnSpacing * 3) / 4
          label: "consumidores"
          value: root.consumerText
          tone: root.ink
        }
      }

      // tok/s history — the dashboard's dot graph
      Item {
        visible: root.showGraph
        width: parent.width
        implicitHeight: graphHead.implicitHeight

        Column {
          id: graphHead
          width: parent.width
          spacing: Style.space(6)

          PanelLabel {
            label: "tok/s · últimas " + Math.round(root.historyLength * root.openPollSeconds / 60) + " min"
            size: Style.font.caption
          }

          Rectangle {
            id: graphFrame
            width: parent.width
            height: Style.space(64)
            radius: root.cardRadius
            color: Util.alpha(root.ink, 0.03)
            border.width: 1
            border.color: Util.alpha(root.ink, 0.12)

            DotGraph {
              anchors.fill: parent
              anchors.margins: Style.space(8)
              slots: root.tpsSeries
              peak: root.tpsPeak
              tone: root.online ? root.okColor : root.dimColor
            }
          }
        }
      }

      Separator { width: parent.width }

      // footer
      Item {
        width: parent.width
        implicitHeight: Math.max(footerLeft.implicitHeight, footerRight.implicitHeight)

        Text {
          id: footerLeft
          textFormat: Text.PlainText
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          text: "clic izq. panel · clic dcho. dashboard · clic central refresca"
          color: Util.alpha(root.dimColor, 0.85)
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }

        Text {
          id: footerRight
          textFormat: Text.PlainText
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          text: root.online ? "cada " + root.openPollSeconds + "s" : root.lastError
          color: Util.alpha(root.dimColor, 0.85)
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }
      }
    }
  }
}
