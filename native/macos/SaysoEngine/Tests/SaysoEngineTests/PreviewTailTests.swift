import XCTest

@testable import SaysoEngine

final class PreviewTailTests: XCTestCase {
    func testNewWordsAreTheTail() {
        var tail = PreviewTail()
        XCTAssertEqual(tail.update("hello", at: 0)?.tentative, "hello")
        let split = tail.update("hello world", at: 2)
        XCTAssertEqual(split?.committed, "hello")
        XCTAssertEqual(split?.tentative, "world")
    }

    func testWordsStayInTheTailForTheWindow() {
        var tail = PreviewTail()
        _ = tail.update("one", at: 0)
        _ = tail.update("one two", at: 0.5)
        let split = tail.update("one two three", at: 0.9)
        XCTAssertEqual(split?.committed, "")
        XCTAssertEqual(split?.tentative, "one two three")
        let later = tail.update("one two three four", at: 1.4)
        XCTAssertEqual(later?.committed, "one")
        XCTAssertEqual(later?.tentative, "two three four")
    }

    func testTheSameTextAgainLetsOldWordsLeave() {
        var tail = PreviewTail()
        _ = tail.update("one", at: 0)
        _ = tail.update("one two", at: 0.5)
        XCTAssertNil(tail.update("one two", at: 0.6), "nothing changed")
        let split = tail.update("one two", at: 1.2)
        XCTAssertEqual(split?.committed, "one")
        XCTAssertEqual(split?.tentative, "two")
    }

    func testTheNewestUpdateIsAlwaysInTheTail() {
        var tail = PreviewTail()
        _ = tail.update("one", at: 0)
        let split = tail.update("one two", at: 5)
        XCTAssertEqual(split?.committed, "one")
        XCTAssertEqual(split?.tentative, "two")
        XCTAssertNil(tail.update("one two", at: 9))
    }

    func testAWordThatGrowsIsNotCut() {
        var tail = PreviewTail()
        _ = tail.update("hello wor", at: 0)
        let split = tail.update("hello world", at: 3)
        XCTAssertEqual(split?.committed, "hello")
        XCTAssertEqual(split?.tentative, "world")
    }

    func testTextWithoutSpacesSplitsBetweenCharacters() {
        var tail = PreviewTail()
        _ = tail.update("今日は", at: 0)
        let split = tail.update("今日はいい天気", at: 3)
        XCTAssertEqual(split?.committed, "今日は")
        XCTAssertEqual(split?.tentative, "いい天気")
    }

    func testFinishCommitsAllOnce() {
        var tail = PreviewTail()
        _ = tail.update("hello world", at: 0)
        XCTAssertEqual(tail.finish("hello world"), "hello world")
        XCTAssertNil(tail.finish("hello world"))
    }
}
