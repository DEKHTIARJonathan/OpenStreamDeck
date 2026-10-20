import { useEffect, useRef, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  CheckCircle2,
  Command,
  ExternalLink,
  Power,
  ShieldCheck,
  Volume2,
} from "lucide-react";
import { call } from "./api";
import { containDialogFocus } from "./UpdatePanel";

interface SetupStatus {
  completed: boolean;
  platform: string;
  input_access: "granted" | "required" | "not_required" | "limited";
  launch_at_login: boolean | null;
  login_error: string | null;
}
const appIcon = new URL("../src-tauri/icons/source.svg", import.meta.url).href;

export default function Onboarding({
  paused,
  menuBarOnly,
}: {
  paused: boolean;
  menuBarOnly: boolean;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const running = useRef(false);
  const alive = useRef(false);
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [step, setStep] = useState(0);
  const [finished, setFinished] = useState(false);
  const [openedSettings, setOpenedSettings] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const run = async (operation: () => Promise<void>) => {
    if (running.current) return;
    running.current = true;
    setPending(true);
    setError("");
    try {
      await operation();
    } catch (e) {
      if (alive.current) setError(String(e));
    } finally {
      running.current = false;
      if (alive.current) setPending(false);
    }
  };
  const refresh = async () => {
    const next = await call<SetupStatus>("onboarding_status");
    if (alive.current) {
      setStatus(next);
      if (next.completed) setFinished(true);
    }
  };
  useEffect(() => {
    alive.current = true;
    let current = true;
    void call<SetupStatus>("onboarding_status")
      .then((next) => {
        if (current) {
          setStatus(next);
          if (next.completed) setFinished(true);
        }
      })
      .catch((e) => {
        if (current) setError(String(e));
      });
    return () => {
      current = false;
      alive.current = false;
    };
  }, []);
  useEffect(() => {
    const element = dialog.current!;
    if (!finished && !paused) {
      if (!element.open) element.showModal();
      heading.current?.focus();
    } else {
      element.close();
      if (finished && !paused)
        document.querySelector<HTMLButtonElement>(".nav.active")?.focus();
    }
    return () => element.close();
  }, [finished, paused, step]);
  useEffect(() => {
    if (paused || finished || !status) return;
    const check = () => {
      if (document.visibilityState === "visible") void run(refresh);
    };
    window.addEventListener("focus", check);
    document.addEventListener("visibilitychange", check);
    return () => {
      window.removeEventListener("focus", check);
      document.removeEventListener("visibilitychange", check);
    };
  }, [paused, finished, step, status?.platform]);
  const finish = (enableLogin: boolean) =>
    run(async () => {
      try {
        await call("finish_onboarding", { enableLogin });
        if (alive.current) setFinished(true);
      } catch (error) {
        // Login can save before completion persistence fails. Reflect the
        // authoritative registration while retaining the original failure.
        try {
          await refresh();
        } catch {
          // The completion error remains the actionable failure.
        }
        throw error;
      }
    });
  const skipStep = () => {
    if (pending) return;
    if (step === 0 && status) {
      setError("");
      setStep(1);
    } else void finish(false);
  };
  const mac = status?.platform === "macos";
  const granted = status?.input_access === "granted";
  const limited = status?.input_access === "limited";
  const loginEnabled = status?.launch_at_login === true;
  const next = () => {
    if (!status) {
      void run(refresh);
      return;
    }
    if (step === 1) {
      if (status.login_error) {
        void run(refresh);
        return;
      }
      void finish(!loginEnabled);
      return;
    }
    if (!mac || granted) {
      setError("");
      setStep(1);
      return;
    }
    if (openedSettings) {
      void run(refresh);
      return;
    }
    void run(async () => {
      await call("open_accessibility_settings");
      if (alive.current) setOpenedSettings(true);
      await refresh();
    });
  };
  const title = !status
    ? "A little setup. A lot within reach."
    : step === 0
      ? mac
        ? "Give your controls a voice."
        : "Make every control count."
      : "Ready when you are.";
  return (
    <dialog
      ref={dialog}
      className="onboarding-dialog"
      aria-labelledby="setup-title"
      aria-describedby="setup-description"
      onCancel={(event) => {
        event.preventDefault();
        skipStep();
      }}
      onKeyDown={containDialogFocus}
    >
      <header className="onboarding-header">
        <div className="onboarding-brand">
          <img src={appIcon} width="34" height="34" alt="" />
          <span>
            OpenStreamDeck<small>QUICK SETUP</small>
          </span>
        </div>
        <button
          className="onboarding-skip"
          disabled={pending}
          onClick={() => void finish(false)}
        >
          Skip setup
        </button>
      </header>
      <div className="onboarding-progress" aria-label={`Step ${step + 1} of 2`}>
        {["Your controls", "Your routine"].map((label, index) => (
          <div
            key={label}
            className={index <= step ? "active" : ""}
            aria-current={step === index ? "step" : undefined}
          >
            <span />
            <small>
              <span aria-hidden="true">{`0${index + 1}`}</span>
              {label}
            </small>
          </div>
        ))}
      </div>
      <div className="onboarding-body">
        <div
          className={`onboarding-symbol ${granted && step === 0 ? "verified" : ""}`}
          aria-hidden="true"
        >
          {step === 1 ? (
            <Power size={30} />
          ) : granted ? (
            <ShieldCheck size={30} />
          ) : (
            <Volume2 size={30} />
          )}
        </div>
        <p className="onboarding-eyebrow">
          STEP {step + 1} OF 2 · {step === 0 ? "YOUR CONTROLS" : "YOUR ROUTINE"}
        </p>
        <h2 id="setup-title" ref={heading} tabIndex={-1}>
          {title}
        </h2>
        <p id="setup-description" className="onboarding-description">
          {!status
            ? "Choose how OpenStreamDeck fits into your day. Both steps are optional."
            : step === 1
              ? "Start OpenStreamDeck when you sign in. Your deck will be there when you need it."
              : mac
                ? "Allow Accessibility access so your deck can adjust volume, control media and send keyboard shortcuts."
                : status.platform === "windows"
                  ? "Windows does not require macOS Accessibility permission for volume, media controls or keyboard shortcuts."
                  : "Linux does not use macOS Accessibility permission. Input controls depend on your desktop session."}
        </p>
        {status && (
          <div className="onboarding-detail" role="status" aria-live="polite">
            {step === 0 ? (
              <>
                {granted ? <CheckCircle2 size={20} /> : <Command size={20} />}
                <div>
                  <strong>
                    {mac
                      ? granted
                        ? "Accessibility access enabled"
                        : "Accessibility access needed"
                      : limited
                        ? "Wayland input is limited"
                        : "No Accessibility request needed"}
                  </strong>
                  <p>
                    {mac
                      ? granted
                        ? "Your Mac reports that OpenStreamDeck has access. You’re ready for the next step."
                        : "In System Settings → Privacy & Security → Accessibility, enable OpenStreamDeck. If it isn’t listed, add the app with the + button, then return here."
                      : status.platform === "windows"
                        ? "Shortcuts may not reach apps running as administrator. Individual actions report errors when access is unavailable."
                        : limited
                          ? "This build uses X11 input. Volume, media keys and shortcuts may not reach Wayland apps. Use an X11 session for these controls."
                          : "X11 input needs a working display connection. N3 USB access also needs the included udev rule; reconnect after installing it."}
                  </p>
                </div>
              </>
            ) : (
              <>
                {loginEnabled ? (
                  <CheckCircle2 size={20} />
                ) : (
                  <Power size={20} />
                )}
                <div>
                  <strong>
                    {loginEnabled
                      ? "Launch at login is enabled"
                      : "An optional head start"}
                  </strong>
                  <p>
                    {status.login_error ??
                      (menuBarOnly
                        ? `Starts in the ${mac ? "menu bar" : "system tray"} and keeps your deck running when you close the window.`
                        : "Opens the editor when you sign in. Closing the window quits the app with your current settings.")}
                  </p>
                </div>
              </>
            )}
          </div>
        )}
        <p className="onboarding-consequence">
          {step === 0
            ? mac && !granted
              ? "Skip this step and volume, media controls and shortcuts may be unavailable. Enable access later in System Settings."
              : "Individual controls can still depend on your OS and desktop session. You can continue and configure your deck."
            : loginEnabled
              ? "Launch at login is already enabled. Manage this anytime in Settings → General."
              : "Skip automatic startup and you’ll need to open OpenStreamDeck before using your deck. Change this anytime in Settings → General."}
        </p>
        {error && (
          <p className="onboarding-error" role="alert">
            {error}
          </p>
        )}
      </div>
      <footer className="onboarding-footer">
        <div className="onboarding-footer-note">
          <ShieldCheck size={14} aria-hidden="true" /> Your choice. On your
          computer.
        </div>
        <div className="onboarding-actions">
          {step === 1 && (
            <button
              className="onboarding-back"
              aria-label="Back to controls"
              disabled={pending}
              onClick={() => {
                setError("");
                setStep(0);
              }}
            >
              <ArrowLeft size={16} />
            </button>
          )}
          <button disabled={pending} onClick={skipStep}>
            {step === 0 ? "Skip for now" : "Not now"}
          </button>
          <button
            className="primary"
            disabled={pending || (!status && !error)}
            onClick={next}
          >
            {pending
              ? "Working…"
              : !status
                ? "Try again"
                : step === 1
                  ? status.login_error
                    ? "Try again"
                    : loginEnabled
                      ? "Finish"
                      : "Enable & finish"
                  : mac && !granted
                    ? openedSettings
                      ? "Check again"
                      : "Open System Settings"
                    : "Continue"}
            {!pending &&
              (step === 0 && mac && !granted && !openedSettings ? (
                <ExternalLink size={15} aria-hidden="true" />
              ) : (
                <ArrowRight size={16} aria-hidden="true" />
              ))}
          </button>
          {step === 0 && openedSettings && mac && !granted && (
            <button
              className="onboarding-reopen"
              disabled={pending}
              onClick={() =>
                void run(async () => {
                  await call("open_accessibility_settings");
                })
              }
            >
              Reopen System Settings
            </button>
          )}
        </div>
      </footer>
    </dialog>
  );
}
