import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Keyboard, Loader2, RadioTower, RefreshCw, Sparkles } from "lucide-react";
import {
  macKeyboardApi,
  type MacKeyboardServicesStatus,
} from "@/lib/api/settings";
import { Button } from "@/components/ui/button";
import { ToggleRow } from "@/components/ui/toggle-row";

type Target = "g610Listening" | "g610Blinking" | "inputMapping";

export function MacKeyboardControlPanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<MacKeyboardServicesStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [busyTarget, setBusyTarget] = useState<Target | null>(null);

  const loadStatus = useCallback(async () => {
    setIsLoading(true);
    try {
      setStatus(await macKeyboardApi.getStatus());
    } catch (error) {
      console.error("[MacKeyboardControlPanel] Failed to load status", error);
      toast.error(String(error));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const setService = useCallback(
    async (target: Target, enabled: boolean) => {
      setBusyTarget(target);
      try {
        const next =
          target === "g610Listening"
            ? await macKeyboardApi.setG610Listening(enabled)
            : target === "g610Blinking"
              ? await macKeyboardApi.setG610Blinking(enabled)
              : await macKeyboardApi.setInputMapping(enabled);
        setStatus(next);
      } catch (error) {
        console.error("[MacKeyboardControlPanel] Failed to toggle service", error);
        toast.error(String(error));
        await loadStatus();
      } finally {
        setBusyTarget(null);
      }
    },
    [loadStatus],
  );

  const unavailableMessage = useMemo(() => {
    if (!status) return null;
    if (!status.supported) {
      return t("settings.advanced.macKeyboard.unsupported", {
        defaultValue: "Mac keyboard controls are only available on macOS.",
      });
    }
    const missing = [];
    if (!status.g610Listening.installed) missing.push("codex-g610-server-*");
    if (!status.inputMapping.installed) missing.push("codex-mac-input-*");
    return missing.length > 0
      ? t("settings.advanced.macKeyboard.missingScripts", {
          defaultValue: "Missing helper scripts: {{scripts}}",
          scripts: missing.join(", "),
        })
      : null;
  }, [status, t]);

  if (isLoading && !status) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        {t("common.loading", { defaultValue: "Loading..." })}
      </div>
    );
  }

  if (!status) return null;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3 rounded-xl bg-muted/40 px-4 py-3 text-xs text-muted-foreground">
        <div className="min-w-0">
          <p className="font-medium text-foreground">
            {t("settings.advanced.macKeyboard.statusTitle", {
              defaultValue: "Current status",
            })}
          </p>
          <p className="break-words">
            {t("settings.advanced.macKeyboard.statusDetail", {
              defaultValue:
                "Listening: {{listening}} · Blinking: {{blinking}} · Mapping: {{mapping}}",
              listening: status.g610Listening.status,
              blinking: status.g610Blinking.status,
              mapping: status.inputMapping.status,
            })}
          </p>
          {unavailableMessage ? (
            <p className="mt-1 break-words text-amber-600 dark:text-amber-400">
              {unavailableMessage}
            </p>
          ) : null}
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          onClick={() => void loadStatus()}
          disabled={isLoading || busyTarget !== null}
          title={t("common.refresh", { defaultValue: "Refresh" })}
          aria-label={t("common.refresh", { defaultValue: "Refresh" })}
        >
          {isLoading ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <RefreshCw className="h-4 w-4" />
          )}
        </Button>
      </div>

      <ToggleRow
        icon={<RadioTower className="h-4 w-4 text-blue-500" />}
        title={t("settings.advanced.macKeyboard.listening")}
        description={t("settings.advanced.macKeyboard.listeningDescription")}
        checked={status.g610Listening.running}
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onCheckedChange={(checked) =>
          void setService("g610Listening", checked)
        }
      />

      <ToggleRow
        icon={<Sparkles className="h-4 w-4 text-amber-500" />}
        title={t("settings.advanced.macKeyboard.blinking")}
        description={t("settings.advanced.macKeyboard.blinkingDescription")}
        checked={status.g610Blinking.running}
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Blinking.installed
        }
        onCheckedChange={(checked) => void setService("g610Blinking", checked)}
      />

      <ToggleRow
        icon={<Keyboard className="h-4 w-4 text-emerald-500" />}
        title={t("settings.advanced.macKeyboard.inputMapping")}
        description={t("settings.advanced.macKeyboard.inputMappingDescription")}
        checked={status.inputMapping.running}
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.inputMapping.installed
        }
        onCheckedChange={(checked) => void setService("inputMapping", checked)}
      />
    </div>
  );
}
