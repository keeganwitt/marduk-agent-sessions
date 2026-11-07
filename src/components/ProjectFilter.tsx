import { useId, useState } from "react";

import {
  matchProjects,
  NO_PROJECT,
  projectLabel,
  type ProjectOption,
} from "../lib/filter";
import { useI18n } from "../i18n/context";
import { shortenPath } from "../lib/format";

/** With hundreds of projects, a full list is no help; typing narrows it instead. */
const SHOWN = 50;

interface Props {
  options: ProjectOption[];
  value: string | null;
  homeDir: string;
  onChange: (project: string | null) => void;
}

function shownValue(value: string, homeDir: string, noProject: string): string {
  return value === NO_PROJECT ? noProject : shortenPath(value, homeDir);
}

/** The "No project" entry also matches its name in the current language. */
function withTranslatedNoProject(
  matches: ProjectOption[],
  options: ProjectOption[],
  text: string,
  noProject: string,
): ProjectOption[] {
  const needle = text.trim().toLowerCase();
  const entry = options.find((option) => option.dir === NO_PROJECT);
  if (
    !entry ||
    matches.includes(entry) ||
    !noProject.toLowerCase().includes(needle)
  )
    return matches;
  return [entry, ...matches];
}

export function ProjectFilter({ options, value, homeDir, onChange }: Props) {
  const { t } = useI18n();
  const noProject = t("table.project.noProject");
  const id = useId();
  const [text, setText] = useState("");
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const matches = open
    ? withTranslatedNoProject(
        matchProjects(options, text, homeDir),
        options,
        text,
        noProject,
      )
    : [];
  const shown = matches.slice(0, SHOWN);

  const close = () => {
    setOpen(false);
    setText("");
    setActive(0);
  };
  const choose = (project: string) => {
    onChange(project);
    close();
  };

  return (
    <div className="field">
      <span id={`${id}-label`}>{t("table.filter.project")}</span>
      <div className="combo">
        <input
          type="text"
          role="combobox"
          aria-labelledby={`${id}-label`}
          aria-expanded={open}
          aria-controls={`${id}-list`}
          aria-autocomplete="list"
          aria-activedescendant={
            open && shown.length > 0 ? `${id}-option-${active}` : undefined
          }
          placeholder={t("table.filter.anyProject")}
          title={value && value !== NO_PROJECT ? value : undefined}
          value={
            open ? text : value ? shownValue(value, homeDir, noProject) : ""
          }
          onFocus={() => setOpen(true)}
          onBlur={close}
          onChange={(event) => {
            setText(event.target.value);
            setActive(0);
            setOpen(true);
          }}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown" || event.key === "ArrowUp") {
              event.preventDefault();
              setOpen(true);
              const step = event.key === "ArrowDown" ? 1 : -1;
              setActive((current) =>
                shown.length === 0
                  ? 0
                  : (current + step + shown.length) % shown.length,
              );
            } else if (event.key === "Enter" && open && shown[active]) {
              event.preventDefault();
              choose(shown[active].dir);
            } else if (event.key === "Escape" && open) {
              event.preventDefault();
              close();
            }
          }}
        />
        {value !== null && (
          <button
            type="button"
            className="combo-clear"
            aria-label={t("table.filter.everyProject")}
            title={t("table.filter.everyProject")}
            onClick={() => onChange(null)}
          >
            ×
          </button>
        )}
        {open && (
          <ul id={`${id}-list`} role="listbox" className="combo-list">
            {shown.map((option, index) => (
              <li
                key={option.dir}
                id={`${id}-option-${index}`}
                role="option"
                aria-selected={index === active}
                title={option.dir === NO_PROJECT ? undefined : option.dir}
                // Keeps focus in the input, whose blur would close the list before the click.
                onMouseDown={(event) => event.preventDefault()}
                onMouseEnter={() => setActive(index)}
                onClick={() => choose(option.dir)}
              >
                <span
                  className={
                    option.dir === NO_PROJECT
                      ? "combo-path no-project"
                      : "combo-path"
                  }
                >
                  {option.dir === NO_PROJECT
                    ? noProject
                    : projectLabel(option.dir, homeDir)}
                </span>
                <span className="combo-count">{option.count}</span>
              </li>
            ))}
            {matches.length === 0 && (
              <li className="combo-note" aria-disabled="true">
                {t("table.filter.noMatches")}
              </li>
            )}
            {matches.length > SHOWN && (
              <li className="combo-note" aria-disabled="true">
                {t("table.filter.more", { count: matches.length - SHOWN })}
              </li>
            )}
          </ul>
        )}
      </div>
    </div>
  );
}
