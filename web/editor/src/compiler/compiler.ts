import { postASN1ExtensionMessage } from '../code/asn1';
import { CompileError, ModuleIdentifier, TypeDefinition, ValueDefinition } from '../wasm-definitions';
import * as webfs from '../webfs';
import * as client from './CompileWorkerClient';

const textDecoder = new TextDecoder();

export type CompileEvent = {
    kind: 'compiling';
} | {
    kind: 'result';
    result: CompileResult;
}

type CompileEventListener = (event: CompileEvent) => void;
const eventListeners: CompileEventListener[] = [];

export interface Declarations {
    modules: ModuleIdentifier[];
    types: TypeDefinition[];
    values: ValueDefinition[];
}

export type CompileResult = {
    kind: 'success';
    declarations: Declarations;
} | {
    kind: 'error';
    errors: CompileError[];
};

export async function getDeclarations(): Promise<CompileResult> {
    const fs = await webfs.getWebFS();

    const errors: CompileError[] = [];
    for (const path of fs.listAll()) {
        if (path.endsWith('.asn')) {
            const data = await fs.readFile(path);
            const error = await client.addSourceFile(path, textDecoder.decode(data));
            if (error !== null) {
                errors.push(error);
            }
        }
    }
    if (errors.length > 0) {
        return { kind: 'error', errors };
    }

    const error = await client.compile();
    if (error !== null) {
        return { kind: 'error', errors: [error] };
    }

    return {
        kind: 'success',
        declarations: {
            modules: await client.listModules(),
            types: await client.listTypes(),
            values: await client.listValues(),
        }
    };
};

export function addEventListener(listener: CompileEventListener) {
    eventListeners.push(listener);
}

export function removeEventListener(listener: CompileEventListener) {
    const index = eventListeners.indexOf(listener);
    if (index !== -1) {
        eventListeners.splice(index, 1);
    }
}

function dispatchEvent(event: CompileEvent) {
    for (const listener of eventListeners) {
        listener(event);
    }
}

(() => {
    webfs.addEventListener(async () => {
        dispatchEvent({
            kind: 'compiling'
        });
        const result = await getDeclarations();
        dispatchEvent({
            kind: 'result',
            result,
        });
        await postASN1ExtensionMessage({
            type: 'custom',
            message: {
                extension: 'asn1',
                type: 'diagnostics',
                errors: result.kind === 'error' ? result.errors : [],
            }
        })
    });
})();
