import Testing
@testable import Jottery

struct DemoSeedConfigTests {

    @Test func returnsNilWithoutSeedFlag() {
        let config = DemoSeedConfig.parse(
            arguments: ["-demo-theme", "dark"],
            environment: ["DEMO_NOTES_PATH": "/tmp/notes.json"]
        )
        #expect(config == nil)
    }

    @Test func returnsNilWithoutNotesPath() {
        let config = DemoSeedConfig.parse(arguments: ["-demo-seed"], environment: [:])
        #expect(config == nil)
    }

    @Test func defaultsToDarkListScreen() {
        let config = DemoSeedConfig.parse(
            arguments: ["-demo-seed"],
            environment: ["DEMO_NOTES_PATH": "/tmp/notes.json"]
        )
        #expect(config == DemoSeedConfig(
            notesPath: "/tmp/notes.json", theme: "dark", screen: .list
        ))
    }

    @Test func parsesThemeAndScreenVariants() {
        let base = ["DEMO_NOTES_PATH": "/tmp/n.json"]
        #expect(DemoSeedConfig.parse(
            arguments: ["-demo-seed", "-demo-theme", "light", "-demo-screen", "list"],
            environment: base
        )?.theme == "light")
        #expect(DemoSeedConfig.parse(
            arguments: ["-demo-seed", "-demo-screen", "note:Welcome"], environment: base
        )?.screen == .note(titleContains: "Welcome"))
        #expect(DemoSeedConfig.parse(
            arguments: ["-demo-seed", "-demo-screen", "search:#recipe"], environment: base
        )?.screen == .search(query: "#recipe"))
        #expect(DemoSeedConfig.parse(
            arguments: ["-demo-seed", "-demo-screen", "sync"], environment: base
        )?.screen == .sync)
        #expect(DemoSeedConfig.parse(
            arguments: ["-demo-seed", "-demo-screen", "lock"], environment: base
        )?.screen == .lock)
    }
}
