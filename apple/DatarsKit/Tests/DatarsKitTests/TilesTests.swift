import Metal
import XCTest
@testable import DatarsKit

/// Serves the repo over a fake `https://tiles.test/` origin, honouring HTTP Range like a CDN.
final class RangeServer: URLProtocol {
    static var root: URL!
    static var ranges = 0
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "tiles.test" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let url = request.url!
        guard let data = try? Data(contentsOf: Self.root.appendingPathComponent(String(url.path.dropFirst()))) else {
            client?.urlProtocol(self, didReceive: HTTPURLResponse(url: url, statusCode: 404, httpVersion: "HTTP/1.1", headerFields: nil)!, cacheStoragePolicy: .notAllowed)
            client?.urlProtocolDidFinishLoading(self)
            return
        }
        var (status, body, headers) = (200, data, [String: String]())
        if let r = request.value(forHTTPHeaderField: "Range"), r.hasPrefix("bytes=") {
            let ab = r.dropFirst(6).split(separator: "-").compactMap { Int($0) }
            let (a, b) = (ab[0], min(ab[1], data.count - 1))
            (status, body) = (206, data.subdata(in: a..<(b + 1)))
            headers["Content-Range"] = "bytes \(a)-\(b)/\(data.count)"
            Self.ranges += 1
        }
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: url, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: body)
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

/// Vector tiles in an app: the chart view fetches the archive's byte ranges itself (HTTP Range),
/// hands them to the engine as they arrive, and ends on the same pixels as the reference render
/// (which read the archive from disk).
@MainActor
final class TilesTests: XCTestCase {
    func testTheViewStreamsTileRangesToTheReferencePixels() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../..").standardized
        RangeServer.root = root
        URLProtocol.registerClass(RangeServer.self)
        defer { URLProtocol.unregisterClass(RangeServer.self) }
        let doc = try String(contentsOf: root.appendingPathComponent("examples/descent/doc.json"), encoding: .utf8)
        let golden = try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("tests/golden/descent/golden.json"))) as! [String: Any]
        let first = (golden["states"] as! [[String: Any]])[0]
        let size = (try JSONSerialization.jsonObject(with: Data(doc.utf8)) as! [String: Any])["size"] as! [String: Double]
        let (w, h) = (size["width"]!, size["height"]!)

        let view = DatarsChartView()
        view.frame = CGRect(x: 0, y: 0, width: w, height: h)
        // The document's archive (`../../assets/tiles/…`) resolves against where the chart lives.
        view.engine.dataBase = URL(string: "https://tiles.test/examples/descent/doc.json")
        view.load(.document(doc))
        // Run the main loop until the view has fetched everything the frame needs.
        let deadline = Date().addingTimeInterval(20)
        var quiet = 0
        while Date() < deadline && quiet < 5 {
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            quiet = RangeServer.ranges > 0 && view.engine.dataRequests().isEmpty ? quiet + 1 : 0
        }
        XCTAssertGreaterThan(RangeServer.ranges, 2, "header, directory and tiles came as ranges")
        XCTAssertTrue(view.engine.dataRequests().isEmpty, "nothing left to fetch: \(view.engine.dataRequests())")
        // The view drew with Metal where there is a GPU; the reference is the CPU's pixels.
        if MTLCreateSystemDefaultDevice() != nil {
            XCTAssertTrue(view.engine.gpuBackend?.hasPrefix("Metal") == true, "frames on Metal: \(view.engine.gpuBackend ?? "CPU pixels")")
        }
        view.engine.detachGPU()
        view.engine.resize(width: w, height: h, scale: 1)
        view.engine.frame(now: 1e6)
        XCTAssertEqual(view.engine.pixelHash(), first["pixels"] as? String, "the streamed frame is the reference frame")
    }
}
