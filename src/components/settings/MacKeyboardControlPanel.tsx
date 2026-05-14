import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  Keyboard,
  Loader2,
  RadioTower,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import {
  macKeyboardApi,
  type MacKeyboardServicesStatus,
} from "@/lib/api/settings";
import { Button } from "@/components/ui/button";
import { ToggleRow } from "@/components/ui/toggle-row";

type Target = "g610Listening" | "g610Blinking" | "inputMapping";
type SliderTarget =
  | "defaultBrightness"
  | "blinkBrightness"
  | "frequencyHz"
  | "burstSeconds"
  | "pauseSeconds";

export function MacKeyboardControlPanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<MacKeyboardServicesStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [busyTarget, setBusyTarget] = useState<Target | SliderTarget | null>(
    null,
  );
  const [sliderValues, setSliderValues] = useState({
    defaultBrightness: 0,
    blinkBrightness: 100,
    frequencyHz: 3,
    burstSeconds: 5,
    pauseSeconds: 15,
  });

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

  useEffect(() => {
    if (!status) return;
    setSliderValues({
      defaultBrightness: status.defaultBrightness ?? 0,
      blinkBrightness: status.blinkBrightness ?? 100,
      frequencyHz: status.frequencyHz ?? 3,
      burstSeconds: status.burstSeconds ?? 5,
      pauseSeconds: status.pauseSeconds ?? 15,
    });
  }, [status]);

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
        console.error(
          "[MacKeyboardControlPanel] Failed to toggle service",
          error,
        );
        toast.error(String(error));
        await loadStatus();
      } finally {
        setBusyTarget(null);
      }
    },
    [loadStatus],
  );

  const setSliderValue = useCallback(
    async (target: SliderTarget, value: number) => {
      const normalized =
        target === "defaultBrightness" || target === "blinkBrightness"
          ? Math.max(0, Math.min(100, Math.round(value)))
          : target === "frequencyHz"
            ? Math.max(0.5, Math.min(10, Math.round(value * 10) / 10))
            : target === "burstSeconds"
              ? Math.max(1, Math.min(60, Math.round(value)))
              : Math.max(0, Math.min(120, Math.round(value)));
      setBusyTarget(target);
      try {
        const next = await (target === "defaultBrightness"
          ? macKeyboardApi.setDefaultBrightness(normalized)
          : target === "blinkBrightness"
            ? macKeyboardApi.setBlinkBrightness(normalized)
            : target === "frequencyHz"
              ? macKeyboardApi.setFrequency(normalized)
              : target === "burstSeconds"
                ? macKeyboardApi.setBurstSeconds(normalized)
                : macKeyboardApi.setPauseSeconds(normalized));
        setStatus(next);
      } catch (error) {
        console.error("[MacKeyboardControlPanel] Failed to set value", error);
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
        onCheckedChange={(checked) => void setService("g610Listening", checked)}
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

      <RangeSlider
        label={t("settings.advanced.macKeyboard.defaultBrightness")}
        description={t(
          "settings.advanced.macKeyboard.defaultBrightnessDescription",
        )}
        value={sliderValues.defaultBrightness}
        min={0}
        max={100}
        step={1}
        suffix="%"
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onChange={(value) =>
          setSliderValues((current) => ({
            ...current,
            defaultBrightness: value,
          }))
        }
        onCommit={(value) => void setSliderValue("defaultBrightness", value)}
      />

      <RangeSlider
        label={t("settings.advanced.macKeyboard.blinkBrightness")}
        description={t(
          "settings.advanced.macKeyboard.blinkBrightnessDescription",
        )}
        value={sliderValues.blinkBrightness}
        min={0}
        max={100}
        step={1}
        suffix="%"
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onChange={(value) =>
          setSliderValues((current) => ({
            ...current,
            blinkBrightness: value,
          }))
        }
        onCommit={(value) => void setSliderValue("blinkBrightness", value)}
      />

      <RangeSlider
        label={t("settings.advanced.macKeyboard.frequency")}
        description={t("settings.advanced.macKeyboard.frequencyDescription")}
        value={sliderValues.frequencyHz}
        min={0.5}
        max={10}
        step={0.5}
        suffix=" Hz"
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onChange={(value) =>
          setSliderValues((current) => ({
            ...current,
            frequencyHz: value,
          }))
        }
        onCommit={(value) => void setSliderValue("frequencyHz", value)}
      />

      <RangeSlider
        label={t("settings.advanced.macKeyboard.burstSeconds")}
        description={t("settings.advanced.macKeyboard.burstSecondsDescription")}
        value={sliderValues.burstSeconds}
        min={1}
        max={60}
        step={1}
        suffix="s"
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onChange={(value) =>
          setSliderValues((current) => ({
            ...current,
            burstSeconds: value,
          }))
        }
        onCommit={(value) => void setSliderValue("burstSeconds", value)}
      />

      <RangeSlider
        label={t("settings.advanced.macKeyboard.pauseSeconds")}
        description={t("settings.advanced.macKeyboard.pauseSecondsDescription")}
        value={sliderValues.pauseSeconds}
        min={0}
        max={120}
        step={1}
        suffix="s"
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onChange={(value) =>
          setSliderValues((current) => ({
            ...current,
            pauseSeconds: value,
          }))
        }
        onCommit={(value) => void setSliderValue("pauseSeconds", value)}
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

interface RangeSliderProps {
  label: string;
  description: string;
  value: number;
  min: number;
  max: number;
  step: number;
  suffix: string;
  disabled: boolean;
  onChange: (value: number) => void;
  onCommit: (value: number) => void;
}

function RangeSlider({
  label,
  description,
  value,
  min,
  max,
  step,
  suffix,
  disabled,
  onChange,
  onCommit,
}: RangeSliderProps) {
  const displayValue = step < 1 ? value.toFixed(1) : String(Math.round(value));

  return (
    <div className="rounded-xl border border-border bg-card/50 p-4 transition-colors hover:bg-muted/50">
      <div className="flex items-center justify-between gap-4">
        <div className="space-y-1">
          <p className="text-sm font-medium leading-none">{label}</p>
          <p className="text-xs text-muted-foreground">{description}</p>
        </div>
        <span className="w-16 text-right text-sm tabular-nums text-muted-foreground">
          {displayValue}
          {suffix}
        </span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        aria-label={label}
        className="mt-4 h-2 w-full cursor-pointer accent-emerald-500 disabled:cursor-not-allowed disabled:opacity-50"
        onChange={(event) => onChange(Number(event.currentTarget.value))}
        onPointerUp={(event) => onCommit(Number(event.currentTarget.value))}
        onKeyUp={(event) => onCommit(Number(event.currentTarget.value))}
        onBlur={(event) => onCommit(Number(event.currentTarget.value))}
      />
    </div>
  );
}
