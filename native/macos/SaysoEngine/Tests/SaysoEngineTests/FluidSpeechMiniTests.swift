import XCTest

@testable import SaysoEngine

final class FluidSpeechMiniTests: XCTestCase {
    private func entry(_ path: String, _ type: String = "file") -> FluidSpeechMini.TreeEntry {
        .init(path: path, type: type, size: 1)
    }

    func testFilterKeepsModelFilesOnly() {
        let kept = FluidSpeechMini.neededFiles([
            entry("Encoder.mlmodelc", "directory"),
            entry("Encoder.mlmodelc/weights/weight.bin"),
            entry("Decoder.mlmodelc/model.mil"),
            entry("parakeet_vocab.json"),
            entry("LICENSE"),
            entry("README.md"),
            entry("assets/logo.png"),
            entry("examples/run.py"),
            entry(".gitattributes"),
            entry("EncoderExtra.mlmodelc/model.mil"),
        ]).map(\.path)
        XCTAssertEqual(
            kept,
            [
                "Encoder.mlmodelc/weights/weight.bin", "Decoder.mlmodelc/model.mil",
                "parakeet_vocab.json", "LICENSE",
            ])
    }

    func testCompleteNeedsAllBundlesAndVocabulary() throws {
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("fsm-test-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        func write(_ path: String) throws {
            let url = folder.appendingPathComponent(path)
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try Data().write(to: url)
        }
        for bundle in FluidSpeechMini.bundles {
            for file in ["coremldata.bin", "model.mil", "weights/weight.bin"] {
                try write("\(bundle)/\(file)")
            }
        }
        XCTAssertFalse(FluidSpeechMini.isComplete(in: folder))
        try write("parakeet_vocab.json")
        XCTAssertTrue(FluidSpeechMini.isComplete(in: folder))
        try FileManager.default.removeItem(at: folder.appendingPathComponent("Encoder.mlmodelc/weights"))
        XCTAssertFalse(FluidSpeechMini.isComplete(in: folder))
    }

    func testFolderAndVersion() {
        XCTAssertEqual(TdtVersion(rawValue: "fluid_mini"), .fluidMini)
        XCTAssertEqual(TdtVersion.fluidMini.folderName, "fluid-speech-mini-coreml")
        XCTAssertEqual(TdtVersion.v2.folderName, TdtVersion.v2.repo?.folderName)
    }

    func testJointLinkIsIdempotent() throws {
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("fsm-link-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let joint = folder.appendingPathComponent("JointDecision.mlmodelc", isDirectory: true)
        try FileManager.default.createDirectory(at: joint, withIntermediateDirectories: true)
        try Data("x".utf8).write(to: joint.appendingPathComponent("model.mil"))
        try FluidSpeechMini.linkJointDecision(in: folder)
        try FluidSpeechMini.linkJointDecision(in: folder)
        let linked = folder.appendingPathComponent("JointDecisionv3.mlmodelc/model.mil")
        XCTAssertTrue(FileManager.default.fileExists(atPath: linked.path))
    }
}
