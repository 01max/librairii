type ExportIdentity = {
    exportSourceType?: string;
    sourceShelfNames?: string[];
    sourceCollectionNames?: string[];
};

export function exportSourceContext(operation: ExportIdentity) {
    const collection = ['shelf', 'shelves', 'collection', 'collections'].includes(operation.exportSourceType ?? '');
    return {
        label: collection ? 'Source collections ·' : 'Source shelves ·',
        names: collection ? operation.sourceCollectionNames ?? operation.sourceShelfNames ?? [] : operation.sourceShelfNames ?? [],
    };
}
