import { useEffect, useRef, useState, type CSSProperties } from "react";
import { useI18n } from "../i18n/context";
import type { SessionRow } from "../lib/types";

interface Props {
  session: SessionRow;
  onShowFiles: (session: SessionRow) => void;
  onReveal: (path: string) => void;
  onCopyId: (session: SessionRow) => void;
}

const MENU_HEIGHT = 110;

/**
 * The table scrolls inside its own container, which would clip an absolutely placed menu on
 * the last rows, so the menu is fixed to the viewport and follows its button when it scrolls.
 */
function placement(button: HTMLElement): CSSProperties {
  const rect = button.getBoundingClientRect();
  const right = window.innerWidth - rect.right;
  return rect.bottom + MENU_HEIGHT > window.innerHeight
    ? { right, bottom: window.innerHeight - rect.top + 4 }
    : { right, top: rect.bottom + 4 };
}

export function RowMenu({ session, onShowFiles, onReveal, onCopyId }: Props) {
  const { t } = useI18n();
  const [style, setStyle] = useState<CSSProperties | null>(null);
  const anchor = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const label = session.title ?? session.id;
  const revealPath = session.files[0] ?? session.database;

  useEffect(() => {
    if (!style) return;
    const close = () => setStyle(null);
    // Clicking a half-hidden last row scrolls it into view, which must not close the menu.
    const follow = () => button.current && setStyle(placement(button.current));
    const onPointer = (event: PointerEvent) => {
      if (!anchor.current?.contains(event.target as Node)) close();
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        close();
        button.current?.focus();
      }
    };
    window.addEventListener("pointerdown", onPointer);
    window.addEventListener("keydown", onKey);
    window.addEventListener("scroll", follow, true);
    window.addEventListener("resize", follow);
    return () => {
      window.removeEventListener("pointerdown", onPointer);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("scroll", follow, true);
      window.removeEventListener("resize", follow);
    };
  }, [style]);

  const choose = (action: () => void) => {
    setStyle(null);
    action();
  };

  return (
    <div className="row-menu" ref={anchor}>
      <button
        ref={button}
        type="button"
        className="row-menu-button"
        aria-haspopup="menu"
        aria-expanded={style !== null}
        aria-label={t("table.rowMenu", { title: label })}
        title={t("table.actions")}
        onClick={(event) =>
          setStyle(style ? null : placement(event.currentTarget))
        }
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="3.5" cy="8" r="1.3" fill="currentColor" />
          <circle cx="8" cy="8" r="1.3" fill="currentColor" />
          <circle cx="12.5" cy="8" r="1.3" fill="currentColor" />
        </svg>
      </button>
      {style && (
        <div className="filter-menu row-menu-list" role="menu" style={style}>
          <button
            type="button"
            role="menuitem"
            onClick={() => choose(() => onShowFiles(session))}
          >
            {t("table.files")}
          </button>
          {revealPath && (
            <button
              type="button"
              role="menuitem"
              onClick={() => choose(() => onReveal(revealPath))}
            >
              {t("dialogs.files.showInFolder")}
            </button>
          )}
          <button
            type="button"
            role="menuitem"
            onClick={() => choose(() => onCopyId(session))}
          >
            {t("table.copyId")}
          </button>
        </div>
      )}
    </div>
  );
}
