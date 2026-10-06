import XCTest
@testable import DatarsKit

final class DatarsKitTests: XCTestCase {
    func testADocumentRendersThroughTheRuntime() {
        let e = DatarsEngine(allowScript: false)
        let r = e.loadDocument(json: #"{"datars":1,"size":{"width":120,"height":80},"scene":{"kind":"shape","geom":{"type":"rect","x":10,"y":10,"w":50,"h":30},"fill":"$accent"}}"#)
        XCTAssertEqual(r["ok"] as? Bool, true)
        e.resize(width: 120, height: 80, scale: 1)
        e.frame(now: 0)
        let px = e.pixels()
        XCTAssertEqual(px.width, 120)
        let i = (20 * px.width + 20) * 4
        XCTAssertEqual(px.data[i], 0x42)
        XCTAssertEqual(px.data[i + 2], 0xd0)
        XCTAssertNotNil(e.status()["tokens"])
    }

    func testDataRequestsAreFulfilledByTheHost() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../..").standardized
        let doc = try String(contentsOf: root.appendingPathComponent("examples/election/doc.json"), encoding: .utf8)
        let e = DatarsEngine(allowScript: true)
        _ = e.loadDocument(json: doc)
        XCTAssertEqual(e.dataRequests().first?["url"] as? String, "count.json", "the live source asks the host")
        let data = try Data(contentsOf: root.appendingPathComponent("examples/election/count.json"))
        XCTAssertTrue(e.provide(source: "count", csvOrJSON: data))
        XCTAssertTrue(e.dataRequests().isEmpty)
    }

    func testAppDataFillsASlotAndWrongRowsAreRefused() throws {
        // The banking example: the chart ships without data; the app hands in the user's rows.
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../..").standardized
        let doc = try String(contentsOf: root.appendingPathComponent("examples/spending/doc.json"), encoding: .utf8)
        let e = DatarsEngine(allowScript: true)
        _ = e.loadDocument(json: doc)
        XCTAssertEqual(e.dataRequests().first?["slot"] as? String, "spending", "the slot asks the app")
        e.resize(width: 390, height: 520, scale: 1)
        e.frame(now: 50)
        let sample = e.pixelHash()
        let rows = #"[{"month":"Apr","category":"Housing","amount":12000},{"month":"Apr","category":"Food","amount":5000},{"month":"May","category":"Housing","amount":12500}]"#
        XCTAssertFalse(e.provide(source: "spending", csvOrJSON: Data(#"[{"m":"Apr"}]"#.utf8)), "rows without the chart's columns are refused")
        XCTAssertTrue(e.provide(source: "spending", csvOrJSON: Data(rows.utf8)))
        e.frame(now: 5000)
        XCTAssertNotEqual(e.pixelHash(), sample, "the user's rows are drawn")
    }

    func testEngineDrawnControlsAreDescribedAndOperable() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../..").standardized
        let doc = try String(contentsOf: root.appendingPathComponent("examples/budget/doc.json"), encoding: .utf8)
        let e = DatarsEngine(allowScript: true)
        _ = e.loadDocument(json: doc)
        e.resize(width: 720, height: 440, scale: 1)
        e.frame(now: 0)
        let c = (e.status()["controls"] as? [[String: Any]])?.first
        XCTAssertEqual(c?["signal"] as? String, "income")
        XCTAssertEqual(c?["min"] as? Double, 10000)
        XCTAssertEqual(c?["max"] as? Double, 80000)
        XCTAssertEqual(c?["step"] as? Double, 1000)
        let before = { e.frame(now: 5); return e.pixelHash() }()
        e.setSignal("income", 60000)
        e.frame(now: 10)
        XCTAssertEqual((e.status()["controls"] as? [[String: Any]])?.first?["value"] as? Double, 60000)
        XCTAssertNotEqual(e.pixelHash(), before, "the chart follows")
    }

    /// A select as a native picker sees it: the choices and what they say, the value now, where the
    /// box is; the picker's choice goes back as JSON.
    static let selectDoc = """
    { "datars": 1, "size": { "width": 320, "height": 200 }, "keys": { "SE": { "name": "Sweden" } },
      "signals": { "country": { "type": "str", "default": "SE" } },
      "scene": { "kind": "group", "key": "root", "children": [
        { "kind": "use", "key": "country", "recipe": "@datars/std/select", "params": { "signal": "country", "options": ["SE", "NO"], "label": "Country" } }] } }
    """

    func testSelectsAreOfferedForANativePicker() throws {
        let e = DatarsEngine(allowScript: true)
        _ = e.loadDocument(json: Self.selectDoc)
        e.resize(width: 320, height: 200, scale: 1)
        e.frame(now: 0)
        let select = { (e.status()["controls"] as? [[String: Any]])?.first { $0["kind"] as? String == "select" } }
        let c = try XCTUnwrap(select())
        XCTAssertEqual(c["signal"] as? String, "country")
        XCTAssertEqual(c["current"] as? String, "SE")
        XCTAssertEqual((c["options"] as? [[String: Any]])?.map { $0["label"] as? String }, ["Sweden", "NO"])
        XCTAssertEqual((c["rect"] as? [Double])?.count, 4)
        XCTAssertTrue(e.setSignal("country", json: "\"NO\""))
        e.frame(now: 5)
        XCTAssertEqual(select()?["current"] as? String, "NO")
        XCTAssertFalse(e.setSignal("country", json: "not json"))
    }

    /// The cursor the engine asks for: a pointing hand over the select, an arrow elsewhere.
    func testTheCursorSaysWhatThePointerCanDo() throws {
        let e = DatarsEngine(allowScript: true)
        _ = e.loadDocument(json: Self.selectDoc)
        e.resize(width: 320, height: 200, scale: 1)
        e.frame(now: 0)
        let c = try XCTUnwrap((e.status()["controls"] as? [[String: Any]])?.first { $0["kind"] as? String == "select" })
        let r = try XCTUnwrap(c["rect"] as? [Double])
        _ = e.pointer(.move, x: r[0] + 20, y: r[1] + 10)
        XCTAssertEqual(e.cursor, .pointer)
        _ = e.pointer(.move, x: 310, y: 190)
        XCTAssertEqual(e.cursor, .arrow)
    }

    #if canImport(UIKit)
    /// On iOS a select is the system's menu: a button over the box whose menu lists the options,
    /// the chosen one checked — and VoiceOver finds that button, not the drawn box.
    @MainActor
    func testASelectOpensTheSystemMenuOnIOS() throws {
        let e = DatarsEngine(allowScript: true)
        _ = e.loadDocument(json: Self.selectDoc)
        e.resize(width: 320, height: 200, scale: 1)
        e.frame(now: 0)
        let view = DatarsChartView(engine: e)
        view.frame = CGRect(x: 0, y: 0, width: 320, height: 200)
        view.updateAccessibility()
        let buttons = view.subviews.compactMap { $0 as? UIButton }.filter { $0.showsMenuAsPrimaryAction }
        let b = try XCTUnwrap(buttons.first, "a menu button over the select")
        let rect = try XCTUnwrap(((e.status()["controls"] as? [[String: Any]])?.first { $0["kind"] as? String == "select" })?["rect"] as? [Double])
        XCTAssertEqual(b.frame, CGRect(x: rect[0], y: rect[1], width: rect[2], height: rect[3]))
        let actions = (b.menu?.children ?? []).compactMap { $0 as? UIAction }
        XCTAssertEqual(actions.map(\.title), ["Sweden", "NO"])
        XCTAssertEqual(actions.map(\.state), [.on, .off], "the chosen one checked")
        let elements = view.accessibilityElements ?? []
        XCTAssertTrue(elements.contains { ($0 as AnyObject) === b }, "VoiceOver gets the menu button")
        XCTAssertFalse(elements.contains { ($0 as? UIAccessibilityElement)?.accessibilityLabel == "Country: Sweden" }, "not the drawn box as well")
    }
    #endif

    func testInteractiveMarksCanBeActivatedWithoutAPointer() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../..").standardized
        let doc = try String(contentsOf: root.appendingPathComponent("examples/dashboard/doc.json"), encoding: .utf8)
        let e = DatarsEngine(allowScript: true)
        _ = e.loadDocument(json: doc)
        e.resize(width: 860, height: 420, scale: 1)
        e.frame(now: 0)
        let items = e.status()["semantics"] as? [[String: Any]] ?? []
        let south = items.first { ($0["label"] as? String)?.hasPrefix("South") == true && $0["actionable"] as? Bool == true }
        XCTAssertNotNil(south)
        let before = e.pixelHash()
        XCTAssertTrue(e.activate(path: south?["path"] as? String ?? ""))
        e.frame(now: 10)
        XCTAssertNotEqual(e.pixelHash(), before, "the dashboard filtered")
    }

    func testSemanticsCarryFrames() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../..").standardized
        let doc = try String(contentsOf: root.appendingPathComponent("examples/votes/doc.json"), encoding: .utf8)
        let e = DatarsEngine(allowScript: true)
        _ = e.loadDocument(json: doc)
        e.resize(width: 720, height: 440, scale: 1)
        e.frame(now: 0)
        let items = e.status()["semantics"] as? [[String: Any]] ?? []
        let bar = items.first { ($0["label"] as? String)?.hasPrefix("Social Democrats") == true }
        let frame = DatarsChartView.frame(bar?["rect"])
        XCTAssertNotNil(frame, "the S bar has a frame: \(String(describing: bar))")
        XCTAssertGreaterThan(frame?.height ?? 0, 100, "a tall bar")
        XCTAssertLessThan(frame?.minX ?? 999, 200, "on the left")
    }
}

/// P1 on Apple platforms: every example state renders to the same pixels as the reference (the
/// goldens written by `datars test` on any machine).
final class DeterminismTests: XCTestCase {
    func testExamplesMatchTheGoldens() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../..").standardized
        for name in ["votes", "riksdag", "business", "flows", "spending"] {
            let doc = try String(contentsOf: root.appendingPathComponent("examples/\(name)/doc.json"), encoding: .utf8)
            let golden = try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("tests/golden/\(name)/golden.json"))) as! [String: Any]
            let states = golden["states"] as! [[String: Any]]
            let size = (try JSONSerialization.jsonObject(with: Data(doc.utf8)) as! [String: Any])["size"] as! [String: Double]
            let e = DatarsEngine(allowScript: true)
            let loaded = e.loadDocument(json: doc)
            XCTAssertEqual(loaded["ok"] as? Bool, true, name)
            XCTAssertEqual((e.status()["diagnostics"] as? [Any])?.count ?? 0, 0, "\(name): \(e.status()["diagnostics"] ?? "")")
            e.resize(width: size["width"]!, height: size["height"]!, scale: 1)
            for (i, st) in states.enumerated() {
                e.goto(i)
                e.frame(now: Double(i) * 100 + 50) // long after any transition
                XCTAssertEqual(e.pixelHash(), st["pixels"] as? String, "\(name) state \(st["name"]!)")
            }
        }
    }
}
