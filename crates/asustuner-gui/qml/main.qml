// AsusTuner —— G-Helper 风格主界面
// 头部：标题 + 后端状态灯(点击授权) + 日志面板开关
// 模式栏：配置方案（后端 profile_list 动态下发，G-Helper 式自定义方案）
// 页签：性能 / 风扇 / GPU / 电池 / 监控
// 底部：实时状态条 + 可折叠后端日志/命令行面板
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import org.guts.AsusTuner.app 1.0

ApplicationWindow {
    id: root
    visible: true
    width: 780
    height: 680
    minimumWidth: 720
    minimumHeight: 560
    title: qsTr("AsusTuner")
    color: palette.window

    property bool showLog: false

    // ---- 配置方案辅助：解析 cfg_list（每行 "名称\t平台\tbuiltin\tactive"）----
    function cfgRows() {
        var lines = tuner.cfg_list.split("\n")
        var arr = []
        for (var i = 0; i < lines.length; i++) {
            var f = lines[i].split("\t")
            if (f.length >= 4 && f[0].length > 0) {
                arr.push({name: f[0], platform: f[1], builtin: f[2] === "1", active: f[3] === "1"})
            }
        }
        return arr
    }
    function platformLabel(p) {
        if (p === "quiet") return qsTr("静音")
        if (p === "performance") return qsTr("性能")
        if (p === "lowpower") return qsTr("低功耗")
        return qsTr("平衡")
    }
    function activeLabel() {
        var rows = cfgRows()
        for (var i = 0; i < rows.length; i++) {
            if (rows[i].active) return rows[i].name
        }
        return platformLabel(tuner.platform_profile)
    }

    // 可拖拽风扇曲线编辑器：8 个点，横轴 40-100°C，纵轴 PWM 0-255
    // 双曲线同图编辑器：CPU（蓝）+ GPU（绿），拖拽时自动抓取最近的点
    component CurveEditor: Rectangle {
        id: ed
        property var cpuPoints: []
        property var gpuPoints: []
        property int maxRpm: 6000   // 满转速参考值（PWM 线性换算显示用）
        readonly property color cpuColor: "#2980b9"
        readonly property color gpuColor: "#27ae60"
        property int dragIndex: -1
        property int dragFan: -1      // 0=CPU 1=GPU -1=未选中
        property Item lockTarget: null // 拖点时冻结的外层 Flickable（防拖点变成滚页面）
        readonly property int pad: 26
        signal dragged(bool isCpu, var newPoints)
        color: Qt.darker(palette.base, 1.03)
        radius: 4
        onCpuPointsChanged: canvas.requestPaint()
        onGpuPointsChanged: canvas.requestPaint()
        onMaxRpmChanged: canvas.requestPaint()

        function rpmOf(pwm) { return Math.round(pwm / 255 * maxRpm) }

        function p2x(t) { return pad + (t - 40) / 60 * (width - 2 * pad) }
        function p2y(p) { return height - pad - p / 255 * (height - 2 * pad) }
        function x2t(x) { return Math.round(40 + (x - pad) / (width - 2 * pad) * 60) }
        function y2p(y) { return Math.round((height - pad - y) / (height - 2 * pad) * 255) }
        // 在两条曲线共 16 个点中找最近的；命中则记录并返回 true
        function pickNearest(x, y) {
            var lists = [cpuPoints, gpuPoints]
            var best = -1, bfan = -1, bd = 1e9
            for (var f = 0; f < 2; f++) {
                for (var i = 0; i < lists[f].length; i++) {
                    var dx = p2x(lists[f][i].t) - x
                    var dy = p2y(lists[f][i].p) - y
                    var d = dx * dx + dy * dy
                    if (d < bd) { bd = d; best = i; bfan = f }
                }
            }
            if (bd < 3600) {
                dragIndex = best
                dragFan = bfan
                canvas.requestPaint()
                return true
            }
            return false
        }
        function drawCurve(ctx, pts, color) {
            ctx.strokeStyle = color
            ctx.lineWidth = 2
            ctx.beginPath()
            for (var i = 0; i < pts.length; i++) {
                var px = p2x(pts[i].t)
                var py = p2y(pts[i].p)
                if (i === 0) ctx.moveTo(px, py)
                else ctx.lineTo(px, py)
            }
            ctx.stroke()
            ctx.fillStyle = color
            for (var j = 0; j < pts.length; j++) {
                ctx.beginPath()
                ctx.arc(p2x(pts[j].t), p2y(pts[j].p), 5, 0, 2 * Math.PI)
                ctx.fill()
            }
        }

        Canvas {
            id: canvas
            anchors.fill: parent
            onPaint: {
                var ctx = getContext("2d")
                ctx.clearRect(0, 0, width, height)
                ctx.strokeStyle = Qt.rgba(0.5, 0.5, 0.5, 0.35)
                ctx.lineWidth = 1
                // 纵向网格：每 1000 RPM 一格并逐格标注（上限不是 1000 整数倍时末段不画线）
                for (var r = 0; r <= ed.maxRpm; r += 1000) {
                    var gy = ed.p2y(r / ed.maxRpm * 255)
                    ctx.beginPath()
                    ctx.moveTo(ed.pad, gy)
                    ctx.lineTo(width - ed.pad, gy)
                    ctx.stroke()
                }
                // 横向网格：每 10°C 一格
                for (var gv = 0; gv <= 6; gv++) {
                    var gx = ed.pad + gv * (width - 2 * ed.pad) / 6
                    ctx.beginPath()
                    ctx.moveTo(gx, ed.pad)
                    ctx.lineTo(gx, height - ed.pad)
                    ctx.stroke()
                }
                ctx.fillStyle = palette.mid
                ctx.font = "10px sans-serif"
                for (var rl = 0; rl <= ed.maxRpm; rl += 1000) {
                    var ly = ed.p2y(rl / ed.maxRpm * 255)
                    ctx.fillText(rl + "", 2, Math.min(height - ed.pad + 4, ly + 4))
                }
                for (var tl = 0; tl <= 6; tl++) {
                    ctx.fillText((40 + tl * 10) + "°",
                                 ed.pad + tl * (width - 2 * ed.pad) / 6 - 9, height - 8)
                }
                // GPU 先画（绿），CPU 后画（蓝）叠上层
                ed.drawCurve(ctx, ed.gpuPoints, ed.gpuColor)
                ed.drawCurve(ctx, ed.cpuPoints, ed.cpuColor)
                // 高亮拖拽点
                if (ed.dragIndex >= 0 && ed.dragFan >= 0) {
                    var pts = ed.dragFan === 0 ? ed.cpuPoints : ed.gpuPoints
                    ctx.strokeStyle = palette.text
                    ctx.lineWidth = 2
                    ctx.beginPath()
                    ctx.arc(ed.p2x(pts[ed.dragIndex].t), ed.p2y(pts[ed.dragIndex].p),
                            8, 0, 2 * Math.PI)
                    ctx.stroke()
                }
            }
        }

        MouseArea {
            anchors.fill: parent
            cursorShape: ed.dragIndex >= 0 ? Qt.ClosedHandCursor : Qt.PointingHandCursor
            onPressed: function(m) {
                // 只有真抓到曲线点才冻结外层滚动（空处按下仍可滚页面）
                if (ed.pickNearest(m.x, m.y) && ed.lockTarget) {
                    ed.lockTarget.interactive = false
                }
            }
            onPositionChanged: function(m) {
                if (ed.dragIndex < 0 || ed.dragFan < 0) return
                var src = ed.dragFan === 0 ? ed.cpuPoints : ed.gpuPoints
                var pts = src.slice()
                var nt = Math.max(40, Math.min(100, ed.x2t(m.x)))
                if (ed.dragIndex > 0) nt = Math.max(nt, pts[ed.dragIndex - 1].t)
                if (ed.dragIndex < pts.length - 1) nt = Math.min(nt, pts[ed.dragIndex + 1].t)
                pts[ed.dragIndex].t = nt
                pts[ed.dragIndex].p = Math.max(0, Math.min(255, ed.y2p(m.y)))
                // 经信号写回源数组，由绑定流回——不打断绑定
                ed.dragged(ed.dragFan === 0, pts)
            }
            onReleased: function() { ed.endDrag() }
            onCanceled: function() { ed.endDrag() }
        }
        // 松手/取消：恢复外层滚动并清除拖拽态
        function endDrag() {
            if (ed.lockTarget) ed.lockTarget.interactive = true
            ed.dragIndex = -1
            ed.dragFan = -1
            canvas.requestPaint()
        }
    }

    AsusTunerObject {
        id: tuner
        Component.onCompleted: {
            connectDbus()
            fanCol.loadFromDevice()
            statsTimer.running = true
            logTimer.running = true
        }
    }

    Timer {
        id: statsTimer
        interval: 3000
        running: false
        repeat: true
        onTriggered: {
            tuner.refresh()
            fanCol.summaryTick++
        }
    }

    Timer {
        id: logTimer
        interval: 400
        running: false
        repeat: true
        onTriggered: {
            var t = tuner.drainBackendLog()
            if (t.length > 0) {
                if (logView.length > 40000) logView.remove(0, 20000)
                logView.append(t)
                logView.cursorPosition = logView.length
            }
        }
    }

    // 方案操作后延迟一拍 refresh：让应答尽快回流 UI（无需等 3s 周期）
    Timer {
        id: cfgRespTimer
        interval: 350
        onTriggered: tuner.refresh()
    }

    // 曲线写后延迟读回：等后端两笔 asusd 写入完成再读，
    // 否则读到的是写前旧曲线，连续应用会显得在两套曲线间来回跳
    Timer {
        id: curveReadbackTimer
        interval: 600
        onTriggered: fanCol.loadFromDevice()
    }

    // ===== 方案管理对话框 =====
    Dialog {
        id: cfgDialog
        modal: true
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: 580
        title: qsTr("配置方案管理")

        contentItem: ColumnLayout {
            spacing: 10

            // 现有方案列表
            Repeater {
                model: root.cfgRows()
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    Label {
                        text: modelData.name
                        color: palette.text
                        font.bold: modelData.active
                        Layout.preferredWidth: 110
                        elide: Text.ElideRight
                    }
                    Label {
                        text: (modelData.builtin ? qsTr("内置 · ") : qsTr("自定义 · "))
                              + root.platformLabel(modelData.platform)
                              + (modelData.active ? qsTr(" · 生效中") : "")
                        color: modelData.active ? "#2ecc71" : palette.mid
                        font.pixelSize: 11
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }
                    Button {
                        text: qsTr("应用")
                        enabled: !modelData.active
                        onClicked: {
                            tuner.cfgApply(modelData.name)
                            cfgRespTimer.restart()
                        }
                    }
                    Button {
                        text: qsTr("存入当前设置")
                        ToolTip.visible: hovered
                        ToolTip.text: qsTr("把当前功率/降压/温度墙/风扇曲线/充电设置覆盖到该方案")
                        onClicked: {
                            tuner.cfgSave(modelData.name, modelData.platform, true)
                            cfgRespTimer.restart()
                        }
                    }
                    Button {
                        text: qsTr("删除")
                        enabled: !modelData.builtin
                        onClicked: {
                            tuner.cfgDelete(modelData.name)
                            cfgRespTimer.restart()
                        }
                    }
                }
            }

            Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid }

            // 新建方案
            Label { text: qsTr("新建方案"); color: palette.text; font.bold: true }
            RowLayout {
                spacing: 8
                Label { text: qsTr("名称"); color: palette.text }
                TextField {
                    id: cfgName
                    Layout.preferredWidth: 150
                    placeholderText: qsTr("如：游戏 / 续航")
                }
                Label { text: qsTr("电源管理方案"); color: palette.text }
                ComboBox {
                    id: cfgPlatform
                    Layout.preferredWidth: 120
                    textRole: "label"
                    model: [
                        {label: qsTr("静音"), value: "quiet"},
                        {label: qsTr("平衡"), value: "balanced"},
                        {label: qsTr("性能"), value: "performance"},
                        {label: qsTr("低功耗"), value: "lowpower"}
                    ]
                }
            }
            RowLayout {
                spacing: 8
                CheckBox {
                    id: cfgSnapshot
                    text: qsTr("包含当前设置（功率墙/降压/风扇曲线/充电）")
                    checked: true
                }
                Item { Layout.fillWidth: true }
                Button {
                    text: qsTr("保存方案")
                    highlighted: true
                    onClicked: {
                        var n = cfgName.text.trim()
                        if (n.length === 0) return
                        tuner.cfgSave(n, cfgPlatform.model[cfgPlatform.currentIndex].value, cfgSnapshot.checked)
                        cfgName.text = ""
                        cfgRespTimer.restart()
                    }
                }
            }
            Label {
                text: qsTr("方案 = 电源管理方案（基座）+ 可选捆绑设置；内置三个同名保存即覆盖，删除需自定义方案")
                color: palette.mid
                font.pixelSize: 11
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ===== 头部 =====
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 12
            spacing: 10
            Label {
                text: "AsusTuner"
                font.pixelSize: 20
                font.bold: true
                color: palette.text
            }
            Label {
                text: tuner.board_name
                color: palette.mid
                font.pixelSize: 12
            }
            Item { Layout.fillWidth: true }
            Label {
                text: tuner.backend_running ? qsTr("root 后端已连接") : qsTr("仅基础功能（功率/降压未授权）")
                color: palette.mid
                font.pixelSize: 12
            }
            // 状态灯 = 授权按钮
            Button {
                id: authDot
                leftPadding: 6; rightPadding: 6
                contentItem: Rectangle {
                    implicitWidth: 14; implicitHeight: 14; radius: 7
                    color: tuner.backend_running ? "#2ecc71" : "#e67e22"
                }
                ToolTip.visible: hovered
                ToolTip.text: tuner.backend_running ? qsTr("后端运行中") : qsTr("点击请求管理员授权")
                onClicked: tuner.startBackend()
            }
            Button {
                id: logToggle
                text: root.showLog ? qsTr("收起面板 ▾") : qsTr("后端日志/终端 ▴")
                onClicked: {
                    root.showLog = !root.showLog
                    // 面板高 210：展开时增高窗口，收起时还原，保证面板完整可见
                    root.height += root.showLog ? 210 : -210
                }
            }
        }

        // ===== 模式栏（配置方案动态渲染；方案多时按钮区横向滚动，不撑破布局）=====
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            Layout.bottomMargin: 4
            spacing: 8
            Label { text: qsTr("配置方案"); color: palette.text; font.pixelSize: 13 }
            Item {
                Layout.fillWidth: true
                implicitHeight: Math.max(40, modeRow.implicitHeight)
                Flickable {
                    id: modeFlick
                    anchors.fill: parent
                    contentWidth: modeRow.implicitWidth
                    contentHeight: height
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    RowLayout {
                        id: modeRow
                        height: modeFlick.height
                        spacing: 8
                        Repeater {
                            model: root.cfgRows()
                            Button {
                                Layout.preferredWidth: Math.min(132, Math.max(76, implicitWidth + 22))
                                highlighted: modelData.active
                                text: modelData.name
                                ToolTip.visible: hovered
                                ToolTip.text: modelData.active
                                    ? qsTr("方案生效中")
                                    : qsTr("应用方案（基座：%1）").arg(root.platformLabel(modelData.platform))
                                onClicked: {
                                    tuner.cfgApply(modelData.name)
                                    cfgRespTimer.restart()
                                }
                            }
                        }
                        Label {
                            visible: tuner.cfg_list.length === 0
                            text: qsTr("等待后端…")
                            color: palette.mid
                            font.pixelSize: 12
                        }
                    }
                    ScrollBar.horizontal: ScrollBar { }
                }
            }
            Label {
                text: qsTr("当前: ") + root.activeLabel()
                color: palette.mid
                font.pixelSize: 12
            }
            Button {
                text: qsTr("管理…")
                onClicked: cfgDialog.open()
            }
        }

        // ===== 页签 =====
        TabBar {
            id: bar

            Layout.fillWidth: true
            TabButton { text: qsTr("性能") }
            TabButton { text: qsTr("风扇") }
            TabButton { text: qsTr("GPU") }
            TabButton { text: qsTr("电池") }
            TabButton { text: qsTr("监控") }
        }

        StackLayout {
            currentIndex: bar.currentIndex
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            Layout.margins: 8

            // ---------- 性能页 ----------
            Flickable {
                contentHeight: perfCol.height
                clip: true
                ScrollBar.vertical: ScrollBar { }
                ColumnLayout {
                    id: perfCol
                    width: parent.width
                    spacing: 10

                    GroupBox {
                        title: tuner.ppt_available ? qsTr("功率墙 (W) — 范围读自 BIOS 固件") : qsTr("功率墙 (mW)")
                        Layout.fillWidth: true

                        // 首次可用时把滑条对齐到固件当前值（+兜底定时同步）
                        Timer {
                            interval: 800
                            running: true
                            repeat: false
                            onTriggered: {
                                if (tuner.ppt_available) {
                                    pl1Slider.value = tuner.ppt_pl1
                                    pl3Slider.value = tuner.ppt_pl3
                                    pl2Slider.value = tuner.ppt_pl2
                                }
                            }
                        }
                        Connections {
                            target: tuner
                            function onPptAvailableChanged() {
                                if (tuner.ppt_available) {
                                    pl1Slider.value = tuner.ppt_pl1
                                    pl3Slider.value = tuner.ppt_pl3
                                    pl2Slider.value = tuner.ppt_pl2
                                }
                            }
                        }

                        ColumnLayout {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            spacing: 8

                            // ---- armoury 模式：瓦特滑条（范围/当前值来自固件）----
                            RowLayout {
                                spacing: 8
                                visible: tuner.ppt_available
                                Layout.fillWidth: true
                                Button {
                                    text: "45W"
                                    onClicked: { pl1Slider.value = 45; pl3Slider.value = Math.min(65, tuner.ppt_max); pl2Slider.value = 45 }
                                }
                                Button {
                                    text: "65W"
                                    onClicked: { pl1Slider.value = 65; pl3Slider.value = Math.min(90, tuner.ppt_max); pl2Slider.value = 65 }
                                }
                                Button {
                                    text: "80W"
                                    onClicked: { pl1Slider.value = 80; pl3Slider.value = Math.min(120, tuner.ppt_max); pl2Slider.value = 80 }
                                }
                                Label { text: qsTr("预设"); color: palette.mid; font.pixelSize: 12 }
                                Item { Layout.fillWidth: true }
                                Button {
                                    text: qsTr("读取固件值")
                                    onClicked: {
                                        pl1Slider.value = tuner.ppt_pl1
                                        pl3Slider.value = tuner.ppt_pl3
                                        pl2Slider.value = tuner.ppt_pl2
                                    }
                                }
                            }

                            RowLayout {
                                spacing: 10
                                visible: tuner.ppt_available
                                Layout.fillWidth: true
                                Label { text: qsTr("持续 PL1"); color: palette.text; Layout.preferredWidth: 64 }
                                Slider {
                                    id: pl1Slider
                                    Layout.fillWidth: true
                                    from: tuner.ppt_min; to: tuner.ppt_max; stepSize: 1
                                }
                                Label { text: pl1Slider.value + " W"; color: palette.text; Layout.preferredWidth: 56 }
                            }
                            RowLayout {
                                spacing: 10
                                visible: tuner.ppt_available
                                Layout.fillWidth: true
                                Label { text: qsTr("瞬时 PL3"); color: palette.text; Layout.preferredWidth: 64 }
                                Slider {
                                    id: pl3Slider
                                    Layout.fillWidth: true
                                    from: tuner.ppt_min; to: tuner.ppt_max; stepSize: 1
                                }
                                Label { text: pl3Slider.value + " W"; color: palette.text; Layout.preferredWidth: 56 }
                            }
                            RowLayout {
                                spacing: 10
                                visible: tuner.ppt_available
                                Layout.fillWidth: true
                                Label { text: qsTr("平均 PL2"); color: palette.text; Layout.preferredWidth: 64 }
                                Slider {
                                    id: pl2Slider
                                    Layout.fillWidth: true
                                    from: tuner.ppt_min; to: tuner.ppt_max; stepSize: 1
                                }
                                Label { text: pl2Slider.value + " W"; color: palette.text; Layout.preferredWidth: 56 }
                            }

                            RowLayout {
                                spacing: 8
                                visible: tuner.ppt_available
                                Layout.fillWidth: true
                                Button {
                                    text: qsTr("应用功率墙")
                                    highlighted: true
                                    onClicked: tuner.setPowerLimits(
                                        pl1Slider.value * 1000,
                                        pl3Slider.value * 1000,
                                        pl2Slider.value * 1000)
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: qsTr("固件当前: PL1 %1W / PL2 %2W / PL3 %3W")
                                        .arg(tuner.ppt_pl1.toFixed(0)).arg(tuner.ppt_pl2.toFixed(0)).arg(tuner.ppt_pl3.toFixed(0))
                                    color: palette.mid
                                    font.pixelSize: 11
                                }
                            }

                            // ---- 兜底模式：SpinBox (mW) ----
                            RowLayout {
                                spacing: 8
                                visible: !tuner.ppt_available
                                Layout.fillWidth: true
                                Button { text: "45W"; onClicked: tuner.setPowerLimits(45000, 65000, 45000) }
                                Button { text: "65W"; onClicked: tuner.setPowerLimits(65000, 90000, 65000) }
                                Button { text: "80W"; onClicked: tuner.setPowerLimits(80000, 120000, 80000) }
                                Label { text: qsTr("预设"); color: palette.mid; font.pixelSize: 12 }
                            }
                            RowLayout {
                                spacing: 8
                                visible: !tuner.ppt_available
                                ColumnLayout {
                                    spacing: 2
                                    Label { text: qsTr("STAPM 持续"); font.pixelSize: 11; color: palette.mid }
                                    SpinBox {
                                        id: stapmBox
                                        from: 5000; to: 150000; stepSize: 1000
                                        editable: true; value: 65000
                                    }
                                }
                                ColumnLayout {
                                    spacing: 2
                                    Label { text: qsTr("FAST 瞬时"); font.pixelSize: 11; color: palette.mid }
                                    SpinBox {
                                        id: fastBox
                                        from: 5000; to: 180000; stepSize: 1000
                                        editable: true; value: 90000
                                    }
                                }
                                ColumnLayout {
                                    spacing: 2
                                    Label { text: qsTr("SLOW 平均"); font.pixelSize: 11; color: palette.mid }
                                    SpinBox {
                                        id: slowBox
                                        from: 5000; to: 150000; stepSize: 1000
                                        editable: true; value: 65000
                                    }
                                }
                                Button {
                                    text: qsTr("应用")
                                    onClicked: tuner.setPowerLimits(stapmBox.value, fastBox.value, slowBox.value)
                                }
                            }
                        }
                    }

                    GroupBox {
                        title: qsTr("CPU 降压 (Curve Optimiser)")
                        Layout.fillWidth: true
                        // AMD 专属（ryzenadj/SMU）；Intel 机型 MSR 普遍锁定，直接隐藏
                        visible: tuner.amd_adj
                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 4
                            RowLayout {
                                Layout.fillWidth: true
                                Slider {
                                    id: uvSlider
                                    Layout.fillWidth: true
                                    from: 0; to: -30; stepSize: 1
                                    value: -20
                                }
                                Label {
                                    text: uvSlider.value === 0 ? qsTr("关闭") : qsTr("%1 (降压)").arg(uvSlider.value)
                                    color: palette.text
                                    Layout.preferredWidth: 90
                                }
                                Button {
                                    text: qsTr("应用")
                                    // 本机 (Dragon Range) 不支持 iGPU 降压，仅发全核
                                    onClicked: tuner.setCpuCurve(uvSlider.value, 0)
                                }
                            }
                            Label {
                                text: qsTr("负值降压 = 降功耗降温度（仅 CPU 全核；iGPU 降压在 GPU 页）；若不稳定请回调至 0")
                                color: palette.mid
                                font.pixelSize: 11
                            }
                        }
                    }

                    GroupBox {
                        title: qsTr("温度墙 / Boost")
                        Layout.fillWidth: true
                        RowLayout {
                            anchors.fill: parent
                            spacing: 12
                            Label { text: qsTr("Tctl 温度墙"); color: palette.text; visible: tuner.amd_adj }
                            SpinBox { id: tctlBox; from: 75; to: 100; value: 90; visible: tuner.amd_adj }
                            Button { text: qsTr("应用"); visible: tuner.amd_adj; onClicked: tuner.setTempLimit(tctlBox.value) }
                            Item { Layout.fillWidth: true }
                            Label { text: qsTr("CPU Boost"); color: palette.text }
                            Switch {
                                id: boostSwitch
                                checked: true
                                onToggled: tuner.setBoost(checked)
                            }
                        }
                    }

                    GroupBox {
                        title: qsTr("键盘灯效 (Aura)")
                        Layout.fillWidth: true
                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 8

                            RowLayout {
                                spacing: 10
                                Label { text: qsTr("效果"); color: palette.text }
                                ComboBox {
                                    id: auraMode
                                    Layout.preferredWidth: 120
                                    model: [qsTr("静态"), qsTr("呼吸"), qsTr("闪烁"), qsTr("彩虹")]
                                }
                                Label { text: qsTr("速度"); color: palette.text }
                                ComboBox {
                                    id: auraSpeed
                                    Layout.preferredWidth: 90
                                    model: [qsTr("慢"), qsTr("中"), qsTr("快")]
                                    currentIndex: 1
                                }
                            }

                            RowLayout {
                                spacing: 10
                                Label { text: qsTr("颜色"); color: palette.text }
                                Row {
                                    spacing: 6
                                    // 预设色块（选中带描边）
                                    Repeater {
                                        model: [
                                            {c: "#ff3b30", n: "红"}, {c: "#ff9500", n: "橙"},
                                            {c: "#ffcc00", n: "黄"}, {c: "#34c759", n: "绿"},
                                            {c: "#00c7be", n: "青"}, {c: "#007aff", n: "蓝"},
                                            {c: "#af52de", n: "紫"}, {c: "#ff2d95", n: "粉"},
                                            {c: "#ffffff", n: "白"}
                                        ]
                                        Rectangle {
                                            width: 26; height: 26; radius: 13
                                            color: modelData.c
                                            border.width: auraSel.sel === modelData.c ? 3 : 1
                                            border.color: auraSel.sel === modelData.c ? palette.text : Qt.darker(modelData.c, 1.3)
                                            MouseArea {
                                                anchors.fill: parent
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: auraSel.sel = modelData.c
                                            }
                                        }
                                    }
                                }
                                // 已选颜色（默认紫红）
                                property string sel: "#af52de"
                                id: auraSel
                            }

                            RowLayout {
                                spacing: 10
                                Label { text: qsTr("亮度"); color: palette.text }
                                Repeater {
                                    model: [qsTr("关闭"), "1", "2", qsTr("最亮")]
                                    Button {
                                        text: modelData
                                        onClicked: tuner.setKbdBrightness(index)
                                    }
                                }
                                Item { Layout.fillWidth: true }
                                Button {
                                    text: qsTr("应用灯效")
                                    highlighted: true
                                    onClicked: {
                                        var speeds = [0xe1, 0xeb, 0xf5]
                                        var col = Qt.color(auraSel.sel)
                                        tuner.setAura(auraMode.currentIndex,
                                                      Math.round(col.r * 255),
                                                      Math.round(col.g * 255),
                                                      Math.round(col.b * 255),
                                                      speeds[auraSpeed.currentIndex])
                                    }
                                }
                            }
                        }
                    }

                    Item { Layout.fillHeight: true }
                }
            }

            // ---------- 风扇页 ----------
            Flickable {
                id: fanFlick
                contentHeight: fanCol.height
                clip: true
                ScrollBar.vertical: ScrollBar { }
                ColumnLayout {
                    id: fanCol
                    width: parent.width
                    spacing: 10

                // 当前编辑的风扇曲线数据（pwm 为 0-255 原始值）
                property var cpuPoints: [
                    {t: 56, p: 10}, {t: 61, p: 15}, {t: 66, p: 70}, {t: 71, p: 120},
                    {t: 76, p: 140}, {t: 80, p: 200}, {t: 85, p: 240}, {t: 97, p: 250}
                ]
                property var gpuPoints: [
                    {t: 56, p: 10}, {t: 61, p: 20}, {t: 66, p: 85}, {t: 71, p: 130},
                    {t: 76, p: 195}, {t: 80, p: 220}, {t: 85, p: 240}, {t: 97, p: 250}
                ]
                property int maxRpm: 6000
                function toRpmStr(pts) {
                    var a = []
                    for (var i = 0; i < pts.length; i++) a.push(Math.round(pts[i].p / 255 * maxRpm))
                    return a.join(",")
                }
                // 变更计数器：驱动"当前生效曲线"标签重新求值
                property int summaryTick: 0

                function toTempStr(pts) {
                    var a = []
                    for (var i = 0; i < pts.length; i++) a.push(pts[i].t)
                    return a.join(",")
                }
                function toPwmStr(pts) {
                    var a = []
                    for (var i = 0; i < pts.length; i++) a.push(pts[i].p)
                    return a.join(",")
                }
                function parseCurve(raw) {
                    // raw 形如 "56,61,..;10,15,.."，解析为点数组
                    var parts = raw.split(";")
                    if (parts.length !== 2) return null
                    var ts = parts[0].split(",")
                    var ps = parts[1].split(",")
                    if (ts.length !== 8 || ps.length !== 8) return null
                    var pts = []
                    for (var i = 0; i < 8; i++) {
                        var t = parseInt(ts[i])
                        var p = parseInt(ps[i])
                        if (isNaN(t) || isNaN(p)) return null
                        pts.push({t: t, p: p})
                    }
                    return pts
                }
                function loadFromDevice() {
                    // 从 asusd 读回当前生效曲线填充编辑器
                    var c = tuner.fanCurveRaw(0)
                    if (c.length > 0) {
                        var cp = parseCurve(c)
                        if (cp !== null) cpuPoints = cp
                    }
                    var g = tuner.fanCurveRaw(1)
                    if (g.length > 0) {
                        var gp = parseCurve(g)
                        if (gp !== null) gpuPoints = gp
                    }
                    summaryTick++
                }

                GroupBox {
                    title: qsTr("曲线预设（CPU / GPU 分开，互不影响）")
                    Layout.fillWidth: true
                    RowLayout {
                        anchors.fill: parent
                        spacing: 8
                        Button {
                            text: qsTr("静音")
                            onClicked: {
                                fanCol.cpuPoints = [{t:50,p:0},{t:60,p:25},{t:65,p:45},{t:70,p:70},{t:75,p:95},{t:80,p:120},{t:90,p:170},{t:97,p:220}]
                                fanCol.gpuPoints = [{t:55,p:0},{t:60,p:20},{t:65,p:40},{t:70,p:60},{t:75,p:85},{t:80,p:110},{t:90,p:160},{t:97,p:210}]
                            }
                        }
                        Button {
                            text: qsTr("均衡")
                            onClicked: {
                                fanCol.cpuPoints = [{t:50,p:20},{t:60,p:45},{t:65,p:70},{t:70,p:100},{t:75,p:130},{t:80,p:165},{t:85,p:200},{t:97,p:235}]
                                fanCol.gpuPoints = [{t:55,p:15},{t:60,p:40},{t:65,p:65},{t:70,p:90},{t:75,p:120},{t:80,p:150},{t:85,p:185},{t:97,p:225}]
                            }
                        }
                        Button {
                            text: qsTr("激进")
                            onClicked: {
                                fanCol.cpuPoints = [{t:45,p:80},{t:55,p:120},{t:65,p:160},{t:70,p:195},{t:75,p:225},{t:80,p:240},{t:85,p:250},{t:97,p:255}]
                                fanCol.gpuPoints = [{t:50,p:60},{t:60,p:100},{t:65,p:140},{t:70,p:175},{t:75,p:205},{t:80,p:230},{t:85,p:245},{t:97,p:255}]
                            }
                        }
                        Item { Layout.fillWidth: true }
                        Label {
                            text: qsTr("GPU 曲线略缓于 CPU（低温区更安静）")
                            color: palette.mid
                            font.pixelSize: 11
                        }
                    }
                }

                GroupBox {
                    title: qsTr("曲线编辑（拖动圆点调整；横轴温度 °C，纵轴 PWM 0-255）")
                    Layout.fillWidth: true
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 6
                        // 图例
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 14
                            Rectangle { width: 14; height: 4; radius: 2; color: editor.cpuColor }
                            Label { text: qsTr("CPU 风扇"); color: palette.text; font.pixelSize: 12 }
                            Rectangle { width: 14; height: 4; radius: 2; color: editor.gpuColor }
                            Label { text: qsTr("GPU 风扇"); color: palette.text; font.pixelSize: 12 }
                            Label {
                                text: qsTr("重合时拖离即可分开")
                                color: palette.mid
                                font.pixelSize: 11
                            }
                            Item { Layout.fillWidth: true }
                            Button {
                                text: qsTr("校准满转速")
                                ToolTip.visible: hovered
                                ToolTip.text: qsTr("风扇将全速运转约 12 秒实测最大转速，结束自动恢复曲线并填入")
                                onClicked: tuner.calibrateFans()
                            }
                            Label { text: qsTr("满转速参考"); color: palette.text; font.pixelSize: 12 }
                            SpinBox {
                                id: maxRpmBox
                                from: 3000; to: 9000; stepSize: 100
                                editable: true; value: 6000
                                onValueChanged: fanCol.maxRpm = value
                            }
                            Connections {
                                target: tuner
                                // CPU/GPU 任一校准值回流都重填（取二者较大值）
                                function onFanCalibCpuChanged() { calibFill() }
                                function onFanCalibGpuChanged() { calibFill() }
                                function calibFill() {
                                    var m = Math.max(Math.round(tuner.fan_calib_cpu),
                                                     Math.round(tuner.fan_calib_gpu))
                                    if (m > 0) maxRpmBox.value = m
                                }
                            }
                        }
                        CurveEditor {
                            id: editor
                            Layout.fillWidth: true
                            Layout.preferredHeight: 360
                            lockTarget: fanFlick
                            maxRpm: fanCol.maxRpm
                            cpuPoints: fanCol.cpuPoints
                            gpuPoints: fanCol.gpuPoints
                            onDragged: function(isCpu, newPoints) {
                                // 写回源数组；绑定会自动刷新编辑器
                                if (isCpu)
                                    fanCol.cpuPoints = newPoints
                                else
                                    fanCol.gpuPoints = newPoints
                            }
                        }
                        Label {
                            text: qsTr("CPU 温度 ") + fanCol.toTempStr(fanCol.cpuPoints)
                                  + qsTr(" / 转速≈ ") + fanCol.toRpmStr(fanCol.cpuPoints) + " RPM\n"
                                  + qsTr("GPU 温度 ") + fanCol.toTempStr(fanCol.gpuPoints)
                                  + qsTr(" / 转速≈ ") + fanCol.toRpmStr(fanCol.gpuPoints) + " RPM"
                            color: palette.mid
                            font.pixelSize: 11
                        }
                        Label {
                            visible: tuner.fan_calib_cpu > 0
                            text: qsTr("校准实测: CPU %1 RPM / GPU %2 RPM（满转速参考已自动填入）")
                                .arg(tuner.fan_calib_cpu.toFixed(0)).arg(tuner.fan_calib_gpu.toFixed(0))
                            color: "#2ecc71"
                            font.pixelSize: 11
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            Button {
                                text: qsTr("应用两条曲线")
                                Layout.fillWidth: true
                                onClicked: {
                                    tuner.setFanCurve(
                                        fanCol.toTempStr(fanCol.cpuPoints), fanCol.toPwmStr(fanCol.cpuPoints),
                                        fanCol.toTempStr(fanCol.gpuPoints), fanCol.toPwmStr(fanCol.gpuPoints))
                                    // 写命令是异步的：延迟读回（asusd 可能钳位），编辑器与图同步
                                    curveReadbackTimer.restart()
                                }
                            }
                            Button {
                                text: qsTr("恢复默认")
                                onClicked: {
                                    tuner.restoreFanCurves()
                                    curveReadbackTimer.restart()
                                }
                            }
                        }
                    }
                }

                GroupBox {
                    title: qsTr("当前生效曲线（asusd 读回）")
                    Layout.fillWidth: true
                    Label {
                        anchors.fill: parent
                        // 依赖 summaryTick：应用/恢复/定时刷新后重新读 asusd
                        text: {
                            var _t = fanCol.summaryTick
                            return tuner.fanCurveSummary()
                        }
                        color: palette.text
                        font.pixelSize: 11
                        font.family: "monospace"
                        wrapMode: Text.Wrap
                    }
                }
                }
            }

            // ---------- GPU 页 ----------
            Flickable {
                contentHeight: gpuCol.height
                clip: true
                ScrollBar.vertical: ScrollBar { }
                ColumnLayout {
                    id: gpuCol
                    width: parent.width
                    spacing: 10

                    Label {
                        visible: !tuner.nv_available
                        text: qsTr("未检测到 dGPU 固件接口（无 N 卡或内核过旧）")
                        color: palette.mid
                    }

                    GroupBox {
                        title: qsTr("GPU 模式（MUX 切换）")
                        Layout.fillWidth: true
                        visible: tuner.nv_available
                        ColumnLayout {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            spacing: 8
                            Label {
                                visible: tuner.gpu_reboot_pending
                                text: qsTr("⚠ 有更改等待重启生效")
                                color: "#e67e22"
                                font.pixelSize: 12
                            }
                            RowLayout {
                                spacing: 8
                                Button {
                                    text: qsTr("省电 (Eco)")
                                    highlighted: tuner.dgpu_off
                                    ToolTip.visible: hovered
                                    ToolTip.text: qsTr("禁用独显，仅核显输出")
                                    onClicked: {
                                        tuner.armourySet("gpu_mux_mode", 0)
                                        tuner.armourySet("dgpu_disable", 1)
                                    }
                                }
                                Button {
                                    text: qsTr("混合 (Hybrid)")
                                    highlighted: !tuner.dgpu_off && tuner.gpu_mux === 0
                                    ToolTip.visible: hovered
                                    ToolTip.text: qsTr("双显卡切换，默认模式")
                                    onClicked: {
                                        tuner.armourySet("gpu_mux_mode", 0)
                                        tuner.armourySet("dgpu_disable", 0)
                                    }
                                }
                                Button {
                                    text: qsTr("独显直连")
                                    highlighted: tuner.gpu_mux === 1
                                    ToolTip.visible: hovered
                                    ToolTip.text: qsTr("独显直驱屏幕，性能最佳")
                                    onClicked: {
                                        tuner.armourySet("gpu_mux_mode", 1)
                                        tuner.armourySet("dgpu_disable", 0)
                                    }
                                }
                            }
                            Label {
                                text: qsTr("当前: MUX=%1（0=混合 1=直连），独显%2，需重启后完全生效")
                                    .arg(tuner.gpu_mux).arg(tuner.dgpu_off ? qsTr("禁用") : qsTr("启用"))
                                color: palette.mid
                                font.pixelSize: 11
                                wrapMode: Text.Wrap
                                Layout.fillWidth: true
                            }
                        }
                    }

                    GroupBox {
                        title: qsTr("dGPU 功耗 / 温度")
                        Layout.fillWidth: true
                        visible: tuner.nv_available
                        ColumnLayout {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            spacing: 8

                            Timer {
                                interval: 800
                                running: true
                                repeat: false
                                onTriggered: {
                                    if (tuner.nv_available) {
                                        nvTempSlider.value = tuner.nv_temp
                                        nvBoostSlider.value = tuner.nv_boost
                                    }
                                }
                            }
                            Connections {
                                target: tuner
                                function onNvAvailableChanged() {
                                    if (tuner.nv_available) {
                                        nvTempSlider.value = tuner.nv_temp
                                        nvBoostSlider.value = tuner.nv_boost
                                    }
                                }
                            }

                            RowLayout {
                                spacing: 10
                                Layout.fillWidth: true
                                Label { text: qsTr("温度墙"); color: palette.text; Layout.preferredWidth: 64 }
                                Slider {
                                    id: nvTempSlider
                                    Layout.fillWidth: true
                                    from: tuner.nv_temp_min; to: tuner.nv_temp_max; stepSize: 1
                                }
                                Label { text: nvTempSlider.value + " °C"; color: palette.text; Layout.preferredWidth: 56 }
                                Button {
                                    text: qsTr("应用")
                                    onClicked: tuner.armourySet("nv_temp_target", nvTempSlider.value)
                                }
                            }
                            RowLayout {
                                spacing: 10
                                Layout.fillWidth: true
                                Label { text: qsTr("动态加速"); color: palette.text; Layout.preferredWidth: 64 }
                                Slider {
                                    id: nvBoostSlider
                                    Layout.fillWidth: true
                                    from: tuner.nv_boost_min; to: tuner.nv_boost_max; stepSize: 1
                                }
                                Label { text: nvBoostSlider.value + " W"; color: palette.text; Layout.preferredWidth: 56 }
                                Button {
                                    text: qsTr("应用")
                                    onClicked: tuner.armourySet("nv_dynamic_boost", nvBoostSlider.value)
                                }
                            }
                            Label {
                                text: qsTr("TGP: %1W（基础 %2W）· 温度墙范围 %3-%4°C")
                                    .arg(tuner.nv_tgp.toFixed(0)).arg(tuner.nv_base.toFixed(0))
                                    .arg(tuner.nv_temp_min).arg(tuner.nv_temp_max)
                                color: palette.mid
                                font.pixelSize: 11
                            }
                        }
                    }

                    GroupBox {
                        title: qsTr("iGPU 降压 (Curve Optimiser)")
                        Layout.fillWidth: true
                        visible: tuner.nv_available
                        ColumnLayout {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            spacing: 4
                            RowLayout {
                                Layout.fillWidth: true
                                Slider {
                                    id: igpuSlider
                                    Layout.fillWidth: true
                                    from: 0; to: -30; stepSize: 1
                                    value: -10
                                }
                                Label {
                                    text: igpuSlider.value === 0 ? qsTr("关闭") : qsTr("%1 (降压)").arg(igpuSlider.value)
                                    color: palette.text
                                    Layout.preferredWidth: 90
                                }
                                Button {
                                    text: qsTr("应用")
                                    onClicked: tuner.setIgpuCurve(igpuSlider.value)
                                }
                            }
                            Label {
                                text: qsTr("iGPU (核显) 降压；是否支持取决于 CPU family——Dragon Range(7940HX) 不支持，应用将报失败")
                                color: palette.mid
                                font.pixelSize: 11
                                wrapMode: Text.Wrap
                                Layout.fillWidth: true
                            }
                        }
                    }

                    Item { Layout.fillHeight: true }
                }
            }

            // ---------- 电池页 ----------
            Flickable {
                contentHeight: batCol.height
                clip: true
                ScrollBar.vertical: ScrollBar { }
                ColumnLayout {
                    id: batCol
                    width: parent.width
                    spacing: 10
                GroupBox {
                    title: qsTr("电池健康")
                    Layout.fillWidth: true
                    GridLayout {
                        anchors.fill: parent
                        columns: 2
                        columnSpacing: 20
                        rowSpacing: 6
                        Label { text: qsTr("健康度"); color: palette.text }
                        Label {
                            text: tuner.battery_health.toFixed(1) + " %"
                            color: tuner.battery_health >= 80 ? "#2ecc71" : "#e67e22"
                        }
                        Label { text: qsTr("循环次数"); color: palette.text }
                        Label { text: tuner.battery_cycles + qsTr(" 次"); color: palette.text }
                        Label { text: qsTr("电压"); color: palette.text }
                        Label { text: tuner.battery_voltage.toFixed(2) + " V"; color: palette.text }
                        Label { text: qsTr("瞬时功率"); color: palette.text }
                        Label { text: tuner.battery_power.toFixed(1) + " W"; color: palette.text }
                    }
                }

                GroupBox {
                    title: qsTr("充电限制（延长电池寿命）")
                    Layout.fillWidth: true
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 8
                        Label {
                            text: qsTr("当前限制: ") + tuner.charge_limit + "%"
                            color: palette.text
                        }
                        RowLayout {
                            spacing: 8
                            Button {
                                Layout.preferredWidth: 100
                                highlighted: tuner.charge_limit === 60
                                text: "60%"
                                onClicked: tuner.setChargeLimit(60)
                            }
                            Button {
                                Layout.preferredWidth: 100
                                highlighted: tuner.charge_limit === 80
                                text: "80%"
                                onClicked: tuner.setChargeLimit(80)
                            }
                            Button {
                                Layout.preferredWidth: 100
                                highlighted: tuner.charge_limit === 100
                                text: "100%"
                                onClicked: tuner.setChargeLimit(100)
                            }
                        }
                        Label {
                            text: qsTr("日常建议 60-80%；100% 为充满即停")
                            color: palette.mid
                            font.pixelSize: 11
                        }
                    }
                }
                Item { Layout.fillHeight: true }
                }
            }

            // ---------- 监控页 ----------
            Flickable {
                contentHeight: monCol.height
                clip: true
                ScrollBar.vertical: ScrollBar { }
                ColumnLayout {
                    id: monCol
                    width: parent.width
                    spacing: 10
                GroupBox {
                    title: qsTr("实时传感器")
                    Layout.fillWidth: true
                    GridLayout {
                        anchors.fill: parent
                        columns: 3
                        columnSpacing: 14
                        rowSpacing: 10

                        Label { text: qsTr("CPU 温度"); color: palette.text }
                        ProgressBar {
                            Layout.preferredWidth: 140
                            from: 0; to: 100
                            value: Math.min(tuner.cpu_temp / 100, 1)
                        }
                        Label { text: tuner.cpu_temp.toFixed(1) + " °C"; color: palette.text }

                        Label { text: qsTr("dGPU 温度"); color: palette.text }
                        ProgressBar {
                            Layout.preferredWidth: 140
                            from: 0; to: 100
                            value: Math.min(tuner.dgpu_temp / 100, 1)
                        }
                        Label { text: tuner.dgpu_temp.toFixed(1) + " °C"; color: palette.text }

                        Label { text: qsTr("CPU 频率"); color: palette.text }
                        ProgressBar {
                            Layout.preferredWidth: 140
                            from: 0; to: 1
                            value: Math.min(tuner.cpu_freq / 5500, 1)
                        }
                        Label { text: tuner.cpu_freq.toFixed(0) + " MHz"; color: palette.text }

                        Label { text: qsTr("CPU 功率"); color: palette.text }
                        ProgressBar {
                            Layout.preferredWidth: 140
                            from: 0; to: 1
                            value: Math.min(tuner.cpu_power / 80, 1)
                        }
                        Label {
                            text: tuner.cpu_power >= 0 ? tuner.cpu_power.toFixed(1) + " W" : qsTr("需授权后端")
                            color: palette.text
                        }

                        Label { text: qsTr("风扇 CPU"); color: palette.text }
                        ProgressBar {
                            Layout.preferredWidth: 140
                            from: 0; to: 1
                            value: Math.min(tuner.fan1_rpm / 6000, 1)
                        }
                        Label { text: tuner.fan1_rpm.toFixed(0) + " RPM"; color: palette.text }

                        Label { text: qsTr("风扇 GPU"); color: palette.text }
                        ProgressBar {
                            Layout.preferredWidth: 140
                            from: 0; to: 1
                            value: Math.min(tuner.fan2_rpm / 6000, 1)
                        }
                        Label { text: tuner.fan2_rpm.toFixed(0) + " RPM"; color: palette.text }

                        Label { text: qsTr("电池"); color: palette.text }
                        Item { width: 1; height: 1 }
                        Label { text: tuner.battery_status; color: palette.text }
                    }
                }
                GroupBox {
                    title: qsTr("设备")
                    Layout.fillWidth: true
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 2
                        Label { text: "主板: " + tuner.board_name; color: palette.text; font.pixelSize: 12 }
                        Label { text: "CPU: " + tuner.cpu_model; color: palette.text; font.pixelSize: 12; elide: Text.ElideRight; Layout.maximumWidth: 640 }
                        Label { text: "GPU: " + tuner.gpu_model; color: palette.text; font.pixelSize: 12; elide: Text.ElideRight; Layout.maximumWidth: 640 }
                    }
                }
                Item { Layout.fillHeight: true }
                }
            }
        }

        // ===== 底部状态条 =====
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 28
            color: Qt.darker(palette.window, 1.1)
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                spacing: 18
                Label { text: qsTr("CPU %1°").arg(tuner.cpu_temp.toFixed(0)); color: palette.text; font.pixelSize: 12 }
                Label {
                    text: tuner.cpu_power >= 0 ? qsTr("%1W").arg(tuner.cpu_power.toFixed(0)) : "—"
                    color: palette.text
                    font.pixelSize: 12
                }
                Label { text: qsTr("GPU %1°").arg(tuner.dgpu_temp.toFixed(0)); color: palette.text; font.pixelSize: 12 }
                Label { text: qsTr("风扇 %1/%2").arg(tuner.fan1_rpm.toFixed(0)).arg(tuner.fan2_rpm.toFixed(0)); color: palette.text; font.pixelSize: 12 }
                Label { text: tuner.battery_status; color: palette.text; font.pixelSize: 12 }
                Item { Layout.fillWidth: true }
                Label {
                    text: tuner.backend_running ? "●" : "○"
                    color: tuner.backend_running ? "#2ecc71" : "#e67e22"
                    font.pixelSize: 14
                }
            }
        }

        // ===== 可折叠后端日志/终端面板 =====
        Rectangle {
            visible: root.showLog
            Layout.fillWidth: true
            Layout.preferredHeight: 200
            color: Qt.darker(palette.base, 1.05)
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 6
                spacing: 4
                Label {
                    text: qsTr("后端进程输出（stdout ◀ / stderr • / 命令 ▶）")
                    color: palette.mid
                    font.pixelSize: 11
                }
                ScrollView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    TextArea {
                        id: logView
                        readOnly: true
                        wrapMode: Text.Wrap
                        font.pixelSize: 11
                        font.family: "monospace"
                        color: palette.text
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    Label { text: "root$"; color: palette.mid; font.pixelSize: 12 }
                    TextField {
                        id: cmdInput
                        Layout.fillWidth: true
                        placeholderText: qsTr("输入命令以 root 执行，如: ryzenadj -i")
                        font.pixelSize: 12
                        onAccepted: doExec()
                        function doExec() {
                            if (text.length > 0) {
                                tuner.backendExec(text)
                                text = ""
                            }
                        }
                    }
                    Button { text: qsTr("执行"); onClicked: cmdInput.doExec() }
                }
            }
        }
    }
}
