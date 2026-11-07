import { useId, useState } from "react";

import { matchProjects, type ProjectOption } from "../lib/filter";
import { shortenPath, tildePath } from "../lib/format";

/** With hundreds of projects, a full list is no help; typing narrows it instead. */
const SHOWN = 50;

interface Props {
  options: ProjectOption[];
  value: string | null;
  homeDir: string;
  onChange: (project: string | null) => void;
}

export function ProjectFilter({ options, value, homeDir, onChange }: Props) {
  const id = useId();
  const [text, setText] = useState("");
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const matches = open ? matchProjects(options, text, homeDir) : [];
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
      <span id={`${id}-label`}>Project</span>
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
          placeholder="Any project"
          title={value ?? undefined}
          value={open ? text : value ? shortenPath(value, homeDir) : ""}
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
            aria-label="Show every project"
            title="Show every project"
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
                title={option.dir}
                // Keeps focus in the input, whose blur would close the list before the click.
                onMouseDown={(event) => event.preventDefault()}
                onMouseEnter={() => setActive(index)}
                onClick={() => choose(option.dir)}
              >
                <span className="combo-path">
                  {tildePath(option.dir, homeDir)}
                </span>
                <span className="combo-count">{option.count}</span>
              </li>
            ))}
            {matches.length === 0 && (
              <li className="combo-note" aria-disabled="true">
                No matching projects
              </li>
            )}
            {matches.length > SHOWN && (
              <li className="combo-note" aria-disabled="true">
                {matches.length - SHOWN} more; keep typing to narrow the list
              </li>
            )}
          </ul>
        )}
      </div>
    </div>
  );
}
