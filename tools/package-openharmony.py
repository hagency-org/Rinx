#!/usr/bin/env python3
"""Install Rinx's native document-export bridge into a cargo-makepad OH project.

Run after `cargo makepad ohos deveco -p rinx`, before hvigor/signing. This touches
only the generated project, preserves signing settings and is safe to repeat.
"""
import argparse
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]


def prepare(project):
    main = project / 'entry/src/main'
    ability = main / 'ets/entryability/EntryAbility.ets'
    declarations = main / 'cpp/types/libentry/Index.d.ts'
    text = ability.read_text()
    if "import { RinxExports }" not in text:
        text = "import { RinxExports } from '../rinx/RinxExports';\n" + text
        text = text.replace('extends UIAbility {', 'extends UIAbility {\n  private rinxExports: RinxExports | undefined = undefined;', 1)
        anchor = '    makepad.onCreate(x);'
        if anchor not in text:
            raise ValueError('Unsupported Makepad EntryAbility; onCreate hook missing')
        text = text.replace(anchor, anchor + '\n    this.rinxExports = new RinxExports(this.context);\n    this.rinxExports.start();', 1)
        anchor = '  onDestroy(): void {'
        if anchor not in text:
            raise ValueError('Unsupported Makepad EntryAbility; onDestroy hook missing')
        text = text.replace(anchor, anchor + '\n    this.rinxExports?.stop();', 1)
    signatures = '''
export const rinxTakeExport: () => string;
export const rinxAuthorizeExport: (id: string) => boolean;
export const rinxWriteExport: (id: string, fd: number) => boolean;
export const rinxFinishExport: (id: string, status: number) => void;
'''
    types = declarations.read_text()
    if 'rinxTakeExport' not in types:
        types += signatures
    dest = main / 'ets/rinx'
    dest.mkdir(exist_ok=True)
    shutil.copy2(ROOT / 'packaging/openharmony/RinxExports.ets', dest / 'RinxExports.ets')
    ability.write_text(text)
    declarations.write_text(types)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project', type=Path, default=ROOT / 'target/makepad-open-harmony/rinx')
    prepare(parser.parse_args().project)
