import styles from "./Resolve.module.css";

interface Props {
  onTakeOurs: () => void;
  onTakeTheirs: () => void;
  onAbort: () => void;
  disabled: boolean;
}

export default function Resolve({ onTakeOurs, onTakeTheirs, onAbort, disabled }: Props) {
  return (
    <footer className={styles.bar}>
      <button className={styles.btn} onClick={onTakeOurs} disabled={disabled}>
        Take Ours
      </button>
      <button className={styles.btn} onClick={onTakeTheirs} disabled={disabled}>
        Take Theirs
      </button>
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
