import UIKit

/// Formatting and quick inserts, above the keyboard.
///
/// On a phone that's an accessory bar. An iPad already has a bar of its own there,
/// with undo and paste, and a second one would stack on top of it. The buttons go
/// in that bar instead, which also shows them in the small floating bar that stands
/// in for it with a hardware keyboard.
@MainActor
enum EditorFormattingBar {

    /// `showsFormatting` is for markdown notes. In anything else, bold is just
    /// stars in the wrong place, but the date is still useful.
    static func install(
        on textView: EditorTextView,
        showsFormatting: Bool,
        idiom: UIUserInterfaceIdiom = UIDevice.current.userInterfaceIdiom
    ) {
        guard textView.showsFormatting != showsFormatting else { return }
        textView.showsFormatting = showsFormatting

        var items = showsFormatting ? MarkdownFormat.allCases.map { button(for: $0, on: textView) } : []
        items.append(insertMenu(on: textView))

        if idiom == .pad {
            // Added to what's there, which is the system's own, replacing only ours.
            let assistant = textView.inputAssistantItem
            let group = UIBarButtonItemGroup(barButtonItems: items, representativeItem: nil)
            assistant.trailingBarButtonGroups.removeAll { $0 === textView.formattingGroup }
            assistant.trailingBarButtonGroups.append(group)
            textView.formattingGroup = group
            return
        }

        let toolbar = textView.formattingToolbar ?? makeToolbar()
        let insert = items.removeLast()
        toolbar.items = items + [.flexibleSpace(), insert]
        textView.formattingToolbar = toolbar
        textView.inputAccessoryView = toolbar
        textView.reloadInputViews()
    }

    private static func makeToolbar() -> UIToolbar {
        let toolbar = UIToolbar(frame: CGRect(x: 0, y: 0, width: 320, height: 44))
        toolbar.autoresizingMask = .flexibleWidth
        toolbar.sizeToFit()
        return toolbar
    }

    private static func button(for format: MarkdownFormat, on textView: EditorTextView) -> UIBarButtonItem {
        let action = UIAction(title: format.title, image: UIImage(systemName: format.symbolName)) { [weak textView] _ in
            textView?.apply(format)
        }
        let item = UIBarButtonItem(primaryAction: action)
        item.accessibilityLabel = format.title
        return item
    }

    private static func insertMenu(on textView: EditorTextView) -> UIBarButtonItem {
        let actions = QuickCommands.all.map { command in
            let action = UIAction(title: command.title, image: UIImage(systemName: command.symbolName)) { [weak textView] _ in
                textView?.insert(command)
            }
            // Says what to type next time.
            action.subtitle = command.trigger
            return action
        }
        let item = UIBarButtonItem(
            title: L.editorInsert,
            image: UIImage(systemName: "plus"),
            menu: UIMenu(children: actions)
        )
        item.accessibilityLabel = L.editorInsert
        return item
    }
}

extension MarkdownFormat {
    var title: String {
        switch self {
        case .bold: return L.editorFormatBold
        case .italic: return L.editorFormatItalic
        case .bulletList: return L.editorFormatList
        case .link: return L.editorFormatLink
        }
    }

    var symbolName: String {
        switch self {
        case .bold: return "bold"
        case .italic: return "italic"
        case .bulletList: return "list.bullet"
        case .link: return "link"
        }
    }
}

extension QuickCommand {
    var title: String {
        switch kind {
        case .dateTime: return L.editorInsertDateTime
        case .date: return L.editorInsertDate
        case .time: return L.editorInsertTime
        case .uuid: return L.editorInsertUUID
        case .text: return L.editorInsertRule
        }
    }

    var symbolName: String {
        switch kind {
        case .dateTime: return "calendar.badge.clock"
        case .date: return "calendar"
        case .time: return "clock"
        case .uuid: return "number"
        case .text: return "minus"
        }
    }
}
