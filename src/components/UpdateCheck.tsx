import { useState } from "react";
import { useI18n } from "../i18n/context";
import {
  fetchLatestRelease,
  isNewer,
  UpdateError,
  type LatestRelease,
} from "../lib/updates";

type Check =
  | { status: "idle" | "checking" | "current" }
  | { status: "available"; release: LatestRelease }
  | { status: "error"; error: UpdateError };

export function UpdateCheck({
  version,
  versionError,
}: {
  version: string | null;
  versionError: boolean;
}) {
  const { t } = useI18n();
  const [check, setCheck] = useState<Check>({ status: "idle" });

  const checkForUpdates = async () => {
    if (!version || check.status === "checking") return;
    setCheck({ status: "checking" });
    try {
      const release = await fetchLatestRelease();
      setCheck(
        isNewer(release.version, version)
          ? { status: "available", release }
          : { status: "current" },
      );
    } catch (error) {
      setCheck({
        status: "error",
        error:
          error instanceof UpdateError
            ? error
            : new UpdateError("updates.networkError"),
      });
    }
  };

  return (
    <div className="help-updates">
      <div className="help-update-controls">
        {version && <span className="help-version">Marduk {version}</span>}
        <button
          type="button"
          disabled={!version || versionError || check.status === "checking"}
          onClick={() => void checkForUpdates()}
        >
          {t(
            check.status === "checking" ? "updates.checking" : "updates.check",
          )}
        </button>
      </div>
      <div role="status">
        {check.status === "current" && <p>{t("updates.current")}</p>}
        {check.status === "available" && (
          <p>
            {t("updates.available", { version: check.release.version })}{" "}
            <a href={check.release.url}>{t("updates.download")}</a>
          </p>
        )}
      </div>
      {(versionError || check.status === "error") && (
        <p className="error" role="alert">
          {versionError
            ? t("updates.versionError")
            : check.status === "error" &&
              t(check.error.key, { status: check.error.status })}
        </p>
      )}
    </div>
  );
}
