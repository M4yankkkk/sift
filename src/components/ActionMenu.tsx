import React, { useState, useEffect, useRef } from "react";
import type { SearchResultItem } from "../types/contract";
import {
  ExternalLink,
  FolderOpen,
  Terminal,
  Copy,
  Check,
  Eye,
  FolderCheck,
  Sliders,
  X,
  Search,
} from "lucide-react";
import { SiftAPI } from "../services/tauriClient";

export interface ActionMenuProps {
  isOpen: boolean;
  onClose: () => void;
  item: SearchResultItem | null;
  previewContent?: string | null;
  onTogglePreview: () => void;
  showPreview: boolean;
  onOpenFolders: () => void;
  onOpenSettings: () => void;
}

interface ActionItem {
  id: string;
  title: string;
  description: string;
  shortcut: string;
  icon: React.ReactNode;
  category: "file" | "view" | "system";
  action: () => void | Promise<void>;
}

export const ActionMenu: React.FC<ActionMenuProps> = ({
  isOpen,
  onClose,
  item,
  previewContent,
  onTogglePreview,
  showPreview,
  onOpenFolders,
  onOpenSettings,
}) => {
  const [filterQuery, setFilterQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [copiedPath, setCopiedPath] = useState(false);
  const [copiedContent, setCopiedContent] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (isOpen) {
      setFilterQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  const actions: ActionItem[] = [
    ...(item
      ? [
          {
            id: "open",
            title:
              item.extension.toLowerCase() === "pdf" && item.page_number
                ? `Open PDF at Page ${item.page_number}`
                : "Open File",
            description:
              item.extension.toLowerCase() === "pdf" && item.page_number
                ? `Launch default PDF viewer directly at page ${item.page_number}`
                : "Open with default system application",
            shortcut: "↵",
            icon: <ExternalLink size={15} color="var(--accent-violet-light)" />,
            category: "file" as const,
            action: () => {
              SiftAPI.openResult(item.path, item.page_number);
              onClose();
            },
          },
          {
            id: "reveal",
            title: "Reveal in File Manager",
            description: "Show location in Nautilus / Dolphin / Files",
            shortcut: "Ctrl+R",
            icon: <FolderOpen size={15} color="var(--accent-amber)" />,
            category: "file" as const,
            action: () => {
              SiftAPI.revealResult(item.path);
              onClose();
            },
          },
          {
            id: "terminal",
            title: "Open in Terminal",
            description: "Spawn terminal in parent directory",
            shortcut: "Ctrl+T",
            icon: <Terminal size={15} color="var(--accent-emerald)" />,
            category: "file" as const,
            action: () => {
              SiftAPI.openInTerminal(item.path);
              onClose();
            },
          },
          {
            id: "copy-path",
            title: copiedPath ? "Path Copied to Clipboard!" : "Copy Absolute Path",
            description: item.path,
            shortcut: "Ctrl+C",
            icon: copiedPath ? (
              <Check size={15} color="var(--accent-emerald)" />
            ) : (
              <Copy size={15} color="var(--text-secondary)" />
            ),
            category: "file" as const,
            action: () => {
              navigator.clipboard.writeText(item.path);
              setCopiedPath(true);
              setTimeout(() => {
                setCopiedPath(false);
                onClose();
              }, 600);
            },
          },
          {
            id: "copy-content",
            title: copiedContent ? "Content Copied!" : "Copy Content / OCR Text",
            description: "Copy file text preview or recognized OCR",
            shortcut: "Ctrl+Shift+C",
            icon: copiedContent ? (
              <Check size={15} color="var(--accent-emerald)" />
            ) : (
              <Copy size={15} color="var(--text-secondary)" />
            ),
            category: "file" as const,
            action: () => {
              const textToCopy = previewContent || item.snippet || item.path;
              navigator.clipboard.writeText(textToCopy);
              setCopiedContent(true);
              setTimeout(() => {
                setCopiedContent(false);
                onClose();
              }, 600);
            },
          },
        ]
      : []),
    {
      id: "toggle-preview",
      title: showPreview ? "Hide Preview Panel" : "Show Preview Panel",
      description: "Toggle expandable side preview pane",
      shortcut: "Tab",
      icon: <Eye size={15} color="var(--accent-cyan)" />,
      category: "view" as const,
      action: () => {
        onTogglePreview();
        onClose();
      },
    },
    {
      id: "folders",
      title: "Manage Index Folders",
      description: "Add or remove indexed directories (~/Documents, etc.)",
      shortcut: "Ctrl+Shift+F",
      icon: <FolderCheck size={15} color="var(--accent-violet-light)" />,
      category: "view" as const,
      action: () => {
        onClose();
        onOpenFolders();
      },
    },
    {
      id: "settings",
      title: "Preferences & Keybindings",
      description: "Configure global hotkey and model settings",
      shortcut: "Ctrl+,",
      icon: <Sliders size={15} color="var(--accent-violet-light)" />,
      category: "view" as const,
      action: () => {
        onClose();
        onOpenSettings();
      },
    },
    {
      id: "dismiss",
      title: "Dismiss Sift",
      description: "Hide launcher window",
      shortcut: "Esc",
      icon: <X size={15} color="var(--text-muted)" />,
      category: "system" as const,
      action: () => {
        onClose();
        SiftAPI.hideWindow().catch(console.error);
      },
    },
  ];

  const filteredActions = actions.filter((act) => {
    if (!filterQuery.trim()) return true;
    const q = filterQuery.toLowerCase();
    return (
      act.title.toLowerCase().includes(q) ||
      act.description.toLowerCase().includes(q) ||
      act.shortcut.toLowerCase().includes(q)
    );
  });

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
      return;
    }

    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) =>
        prev < filteredActions.length - 1 ? prev + 1 : 0
      );
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) =>
        prev > 0 ? prev - 1 : filteredActions.length - 1
      );
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (filteredActions[selectedIndex]) {
        filteredActions[selectedIndex].action();
      }
    }
  };

  if (!isOpen) return null;

  return (
    <div className="action-menu-backdrop" onClick={onClose}>
      <div
        className="action-menu-dialog"
        onClick={(e) => e.stopPropagation()}
        ref={menuRef}
      >
        {/* Action Search Header */}
        <div className="action-menu-header">
          <div className="action-menu-search-wrapper">
            <Search size={14} className="action-menu-search-icon" />
            <input
              ref={inputRef}
              type="text"
              className="action-menu-input"
              placeholder={item ? `Actions for "${item.filename}"...` : "Search actions..."}
              value={filterQuery}
              onChange={(e) => {
                setFilterQuery(e.target.value);
                setSelectedIndex(0);
              }}
              onKeyDown={handleKeyDown}
            />
          </div>
          <button className="action-menu-close-btn" onClick={onClose} title="Close actions (Esc)">
            <X size={14} />
          </button>
        </div>

        {/* Target Item Breadcrumb Subheader */}
        {item && (
          <div className="action-menu-target-bar">
            <span className="action-menu-target-badge">{item.extension.toUpperCase()}</span>
            <span className="action-menu-target-path" title={item.path}>
              {item.path}
            </span>
          </div>
        )}

        {/* Action Options List */}
        <div className="action-menu-list">
          {filteredActions.length === 0 ? (
            <div className="action-menu-empty">No matching actions found</div>
          ) : (
            filteredActions.map((act, idx) => {
              const isSelected = idx === selectedIndex;
              return (
                <div
                  key={act.id}
                  className={`action-menu-item ${isSelected ? "selected" : ""}`}
                  onClick={() => act.action()}
                  onMouseEnter={() => setSelectedIndex(idx)}
                >
                  <div className="action-item-icon-box">{act.icon}</div>
                  <div className="action-item-text">
                    <div className="action-item-title">{act.title}</div>
                    <div className="action-item-desc">{act.description}</div>
                  </div>
                  <span className="keycap action-item-keycap">{act.shortcut}</span>
                </div>
              );
            })
          )}
        </div>

        {/* Footer info */}
        <div className="action-menu-footer">
          <span>Use <span className="keycap-subtle">↑</span> <span className="keycap-subtle">↓</span> to navigate</span>
          <span><span className="keycap-subtle">↵</span> to execute</span>
          <span><span className="keycap-subtle">Esc</span> to dismiss</span>
        </div>
      </div>
    </div>
  );
};
