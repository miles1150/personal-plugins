/** Button component for Miles Works review v1. */
export interface ButtonProps { label: string; variant?: "primary" | "secondary" | "accent" | "danger"; disabled?: boolean; onClick?: () => void; }
export declare function Button(props: ButtonProps): any;
