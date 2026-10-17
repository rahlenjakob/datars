import XCTest

/// The iOS sample app (dev.datars.sample, installed in the simulator first) on one chart, by its
/// URL (`TEST_RUNNER_DATARS_URL`): Xcode's accessibility audit over it — contrast, hit regions,
/// element detection, descriptions, Dynamic Type, clipped text, traits — and the element tree as
/// XCUITest sees it, printed. Any issue fails, each one printed. Run by scripts/test-ios-audit.sh.
final class AuditTests: XCTestCase {
    func testTheChartPassesTheAccessibilityAudit() throws {
        let url = ProcessInfo.processInfo.environment["DATARS_URL"] ?? "http://127.0.0.1:8800/c/votes"
        let app = XCUIApplication(bundleIdentifier: "dev.datars.sample")
        app.launchEnvironment = ["DATARS_URL": url]
        app.launch()
        // (The chart loads over HTTP: its name is on the view once it has.)
        XCTAssertTrue(app.otherElements.element(boundBy: 0).waitForExistence(timeout: 20))
        sleep(6)
        print("AUDIT-TREE \(url)\n\(app.debugDescription)\nAUDIT-TREE-END")
        var issues: [String] = []
        try app.performAccessibilityAudit(for: .all) { issue in
            let at = issue.element.map { "'\($0.label)' \($0.frame)" } ?? "-"
            issues.append("\(issue.compactDescription): \(issue.detailedDescription) — \(at)")
            return true // every issue said, not only the first
        }
        for i in issues { print("AUDIT-ISSUE \(i)") }
        print("AUDIT-COUNT \(issues.count)")
        XCTAssertEqual(issues, [], "the audit's issues")
        app.terminate()
    }
}
