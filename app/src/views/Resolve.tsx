import styles from "./Resolve.module.css";

interface Props {
  onTakeOurs: () => void;
  onTakeTheirs: () => void;
  onTakeBoth?: () => void;
  onAbort: () => void;
  disabled: boolean;
  bothLabel?: string;
}

export default function Resolve({ onTakeOurs, onTakeTheirs, onTakeBoth, onAbort, disabled, bothLabel }: Props) {
  return (
    <footer className={styles.bar}>
      <button className={styles.btn} onClick={onTakeOurs} disabled={disabled}>
        Take Ours
      </button>
      <button className={styles.btn} onClick={onTakeTheirs} disabled={disabled}>
        Take Theirs
      </button>
      {onTakeBoth && (
        <button
          className={`${styles.btn} ${styles.both}`}
          onClick={onTakeBoth}
          disabled={disabled}
        >
          {bothLabel ?? "Take Both"}
        </button>
      )}
      <span className={styles.spacer} />
      <button
        className={`${styles.btn} ${styles.abort}`}
        onClick={onAbort}
        disabled={disabled}
      >
        Abort
      </button>
    </footer>
  );
}
