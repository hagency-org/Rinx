import common from '@ohos.app.ability.common';
import picker from '@ohos.file.picker';
import fs from '@ohos.file.fs';
import native from 'libmakepad.so';

interface ExportRequest {
  id: string;
  name: string;
}

// Host UI only: this module is packaged with Rinx, never evaluated by Splash.
export class RinxExports {
  private timer: number = -1;
  private busy: boolean = false;
  private active: string = '';
  private context: common.UIAbilityContext;
  constructor(context: common.UIAbilityContext) { this.context = context; }

  start(): void {
    if (this.timer !== -1) { return; }
    this.timer = setInterval(() => { this.poll(); }, 250);
  }

  stop(): void {
    if (this.timer !== -1) { clearInterval(this.timer); this.timer = -1; }
    if (this.active !== '') { native.rinxFinishExport(this.active, -1); this.active = ''; }
  }

  private async poll(): Promise<void> {
    if (this.busy) { return; }
    const spec: string = native.rinxTakeExport();
    if (spec === '') { return; }
    this.busy = true;
    const request = JSON.parse(spec) as ExportRequest;
    this.active = request.id;
    let status = -1;
    try {
      const dialog = new picker.DocumentViewPicker(this.context);
      const options = new picker.DocumentSaveOptions();
      options.newFileNames = [request.name];
      const uris = await dialog.save(options);
      if (uris.length === 0) {
        status = 0;
      } else if (native.rinxAuthorizeExport(request.id)) {
        // No truncate flag: an account change before the Rust write must not
        // destroy an existing file selected in the provider.
        const file = fs.openSync(uris[0], fs.OpenMode.READ_WRITE | fs.OpenMode.CREATE);
        try {
          status = native.rinxWriteExport(request.id, file.fd) ? 1 : -1;
        } finally {
          fs.closeSync(file);
        }
      }
    } catch (_) {
      status = -1;
    } finally {
      native.rinxFinishExport(request.id, status);
      this.active = '';
      this.busy = false;
    }
  }
}
