# Fan Composer agent rules

- Windows background commands must use node_repl child_process with windowsHide:true, shell:false and piped output. Never enable the shell tool or change interactive terminal settings.
- Use Rust native UI and audio; no WebView. Keep raw microphone audio in memory only.
- No audio autoplay. Start is explicit; default gain low; stop on device errors and nonfinite DSP values.
- Real-time callbacks: no heap allocation, filesystem I/O, locks, model compilation or waits.
- Document hypotheses, experiments, failures and actual execution devices. Enumeration is not inference validation.
- Do not claim subjective comfort, NPU benefit, acoustic feedback cancellation or long-run stability without measurements.
- Preserve existing code and system settings. Use local build artifacts and dependencies.
