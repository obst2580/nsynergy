import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

interface PairingDialogProps {
  mode: "host" | "join";
  deviceName: string;
  onClose: () => void;
  onPaired: () => void;
}

function PairingDialog({ mode, deviceName, onClose, onPaired }: PairingDialogProps) {
  const [pairingCode, setPairingCode] = useState("");
  const [inputCode, setInputCode] = useState(["", "", "", "", "", ""]);
  const [status, setStatus] = useState<"idle" | "verifying" | "success" | "error">("idle");
  const [errorMessage, setErrorMessage] = useState("");
  const inputRefs = useRef<(HTMLInputElement | null)[]>([]);

  useEffect(() => {
    if (mode === "host") {
      generateCode();
    }
  }, [mode]);

  async function generateCode() {
    try {
      const code = await invoke<string>("generate_pairing_code");
      setPairingCode(code);
    } catch (_e) {
      setErrorMessage("Failed to generate pairing code");
    }
  }

  function handleDigitChange(index: number, value: string) {
    if (!/^\d?$/.test(value)) return;

    const updated = inputCode.map((digit, i) => (i === index ? value : digit));
    setInputCode(updated);

    if (value && index < 5) {
      inputRefs.current[index + 1]?.focus();
    }
  }

  function handleKeyDown(index: number, e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Backspace" && !inputCode[index] && index > 0) {
      inputRefs.current[index - 1]?.focus();
    }
  }

  function handlePaste(e: React.ClipboardEvent) {
    e.preventDefault();
    const pasted = e.clipboardData.getData("text").replace(/\D/g, "").slice(0, 6);
    if (pasted.length === 0) return;

    const updated = inputCode.map((_, i) => (i < pasted.length ? pasted[i] : ""));
    setInputCode(updated);

    const focusIndex = Math.min(pasted.length, 5);
    inputRefs.current[focusIndex]?.focus();
  }

  async function handleVerify() {
    const code = inputCode.join("");
    if (code.length !== 6) {
      setErrorMessage("Please enter all 6 digits");
      return;
    }

    setStatus("verifying");
    setErrorMessage("");

    // Simulate verification delay - in production this would verify with the server
    await new Promise((resolve) => setTimeout(resolve, 1500));

    // For now, any valid 6-digit code succeeds
    // In production, this would call invoke("verify_pairing", { code, deviceName })
    setStatus("success");
    setTimeout(() => {
      onPaired();
    }, 1000);
  }

  function renderHostMode() {
    return (
      <>
        <p style={styles.instruction}>
          Share this code with the device you want to connect:
        </p>
        <div style={styles.codeDisplay}>
          {pairingCode.split("").map((digit, i) => (
            <span key={i} style={styles.codeDigit}>
              {digit}
            </span>
          ))}
        </div>
        <p style={styles.hint}>
          Waiting for <strong>{deviceName}</strong> to enter this code...
        </p>
        <div style={styles.spinner} />
      </>
    );
  }

  function renderJoinMode() {
    if (status === "success") {
      return (
        <div style={styles.successContainer}>
          <div style={styles.checkmark}>&#10003;</div>
          <p style={styles.successText}>
            Successfully paired with <strong>{deviceName}</strong>
          </p>
        </div>
      );
    }

    return (
      <>
        <p style={styles.instruction}>
          Enter the 6-digit code shown on <strong>{deviceName}</strong>:
        </p>
        <div style={styles.codeInput} onPaste={handlePaste}>
          {inputCode.map((digit, i) => (
            <input
              key={i}
              ref={(el) => { inputRefs.current[i] = el; }}
              style={{
                ...styles.digitInput,
                ...(digit ? styles.digitInputFilled : {}),
              }}
              type="text"
              inputMode="numeric"
              maxLength={1}
              value={digit}
              onChange={(e) => handleDigitChange(i, e.target.value)}
              onKeyDown={(e) => handleKeyDown(i, e)}
              disabled={status === "verifying"}
            />
          ))}
        </div>
        {errorMessage && <p style={styles.error}>{errorMessage}</p>}
        <button
          onClick={handleVerify}
          disabled={status === "verifying" || inputCode.join("").length !== 6}
          style={{
            ...styles.verifyBtn,
            ...(status === "verifying" || inputCode.join("").length !== 6
              ? styles.verifyBtnDisabled
              : {}),
          }}
        >
          {status === "verifying" ? "Verifying..." : "Verify & Connect"}
        </button>
      </>
    );
  }

  return (
    <div style={styles.overlay} onClick={onClose}>
      <div style={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <div style={styles.dialogHeader}>
          <h3 style={styles.dialogTitle}>
            {mode === "host" ? "Pair Device" : "Enter Pairing Code"}
          </h3>
          <button onClick={onClose} style={styles.closeBtn}>
            &#10005;
          </button>
        </div>
        <div style={styles.dialogBody}>
          {mode === "host" ? renderHostMode() : renderJoinMode()}
        </div>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  overlay: {
    position: "fixed",
    inset: 0,
    background: "rgba(0, 0, 0, 0.6)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    zIndex: 1000,
  },
  dialog: {
    background: "#1a1a2e",
    borderRadius: "12px",
    border: "1px solid #333",
    width: "100%",
    maxWidth: "400px",
    margin: "16px",
    overflow: "hidden",
  },
  dialogHeader: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "16px 20px",
    borderBottom: "1px solid #333",
  },
  dialogTitle: {
    fontSize: "16px",
    fontWeight: 600,
    color: "#e0e0e0",
    margin: 0,
  },
  closeBtn: {
    background: "transparent",
    border: "none",
    color: "#666",
    fontSize: "16px",
    cursor: "pointer",
    padding: "4px",
    lineHeight: 1,
  },
  dialogBody: {
    padding: "24px 20px",
    textAlign: "center" as const,
  },
  instruction: {
    fontSize: "13px",
    color: "#aaa",
    marginBottom: "20px",
    lineHeight: 1.5,
  },
  codeDisplay: {
    display: "flex",
    justifyContent: "center",
    gap: "8px",
    marginBottom: "24px",
  },
  codeDigit: {
    width: "48px",
    height: "56px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    background: "#16213e",
    borderRadius: "8px",
    border: "1px solid #6c63ff",
    fontSize: "24px",
    fontWeight: 700,
    color: "#6c63ff",
    fontFamily: "monospace",
  },
  hint: {
    fontSize: "12px",
    color: "#666",
    marginBottom: "16px",
  },
  spinner: {
    width: "24px",
    height: "24px",
    border: "3px solid #333",
    borderTopColor: "#6c63ff",
    borderRadius: "50%",
    margin: "0 auto",
    animation: "spin 1s linear infinite",
  },
  codeInput: {
    display: "flex",
    justifyContent: "center",
    gap: "8px",
    marginBottom: "20px",
  },
  digitInput: {
    width: "44px",
    height: "52px",
    textAlign: "center" as const,
    fontSize: "22px",
    fontWeight: 700,
    fontFamily: "monospace",
    background: "#16213e",
    border: "1px solid #444",
    borderRadius: "8px",
    color: "#e0e0e0",
    outline: "none",
    caretColor: "#6c63ff",
  },
  digitInputFilled: {
    borderColor: "#6c63ff",
  },
  error: {
    fontSize: "12px",
    color: "#f44336",
    marginBottom: "12px",
  },
  verifyBtn: {
    width: "100%",
    padding: "12px",
    borderRadius: "8px",
    border: "none",
    background: "#6c63ff",
    color: "#fff",
    fontSize: "14px",
    fontWeight: 600,
    cursor: "pointer",
  },
  verifyBtnDisabled: {
    opacity: 0.5,
    cursor: "not-allowed",
  },
  successContainer: {
    padding: "20px 0",
  },
  checkmark: {
    width: "48px",
    height: "48px",
    borderRadius: "50%",
    background: "#4caf50",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    fontSize: "24px",
    color: "#fff",
    margin: "0 auto 16px",
  },
  successText: {
    fontSize: "14px",
    color: "#e0e0e0",
  },
};

export default PairingDialog;
