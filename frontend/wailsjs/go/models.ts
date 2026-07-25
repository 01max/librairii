export namespace app {
	
	export class APIError {
	    code: string;
	    message: string;
	    details?: Record<string, string>;
	
	    static createFrom(source: any = {}) {
	        return new APIError(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.code = source["code"];
	        this.message = source["message"];
	        this.details = source["details"];
	    }
	}
	export class LibraryPageResponse {
	    page?: library.Page;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new LibraryPageResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.page = this.convertValues(source["page"], library.Page);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class MutationResponse {
	    success: boolean;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new MutationResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.success = source["success"];
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class MetadataStatusResponse {
	    status: metadata.CatalogStatus;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new MetadataStatusResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.status = this.convertValues(source["status"], metadata.CatalogStatus);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class OperationListResponse {
	    operations: operations.Snapshot[];
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new OperationListResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.operations = this.convertValues(source["operations"], operations.Snapshot);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class OperationResponse {
	    operation?: operations.Snapshot;
	    cancelled?: boolean;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new OperationResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.operation = this.convertValues(source["operation"], operations.Snapshot);
	        this.cancelled = source["cancelled"];
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class RemovalResponse {
	    result?: removal.Result;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new RemovalResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.result = this.convertValues(source["result"], removal.Result);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class Status {
	    state: string;
	    startedAt?: string;
	    mutationsAllowed: boolean;
	
	    static createFrom(source: any = {}) {
	        return new Status(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.state = source["state"];
	        this.startedAt = source["startedAt"];
	        this.mutationsAllowed = source["mutationsAllowed"];
	    }
	}
	export class StatusResponse {
	    status: Status;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new StatusResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.status = this.convertValues(source["status"], Status);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class StoryDetailResponse {
	    detail?: library.StoryDetail;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new StoryDetailResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.detail = this.convertValues(source["detail"], library.StoryDetail);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class TagAssignmentResponse {
	    result?: tagging.AssignmentResult;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new TagAssignmentResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.result = this.convertValues(source["result"], tagging.AssignmentResult);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class TagAssignmentWorkspaceResponse {
	    workspace?: tagging.AssignmentWorkspace;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new TagAssignmentWorkspaceResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.workspace = this.convertValues(source["workspace"], tagging.AssignmentWorkspace);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class TagCatalogResponse {
	    catalog?: tagging.Catalog;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new TagCatalogResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.catalog = this.convertValues(source["catalog"], tagging.Catalog);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class TagDefinitionDeletionPlanResponse {
	    plan?: tagging.DefinitionDeletionPlan;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new TagDefinitionDeletionPlanResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.plan = this.convertValues(source["plan"], tagging.DefinitionDeletionPlan);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class TagDefinitionResponse {
	    definition?: tagging.Definition;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new TagDefinitionResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.definition = this.convertValues(source["definition"], tagging.Definition);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class TagValueDeletionPlanResponse {
	    plan?: tagging.ValueDeletionPlan;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new TagValueDeletionPlanResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.plan = this.convertValues(source["plan"], tagging.ValueDeletionPlan);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class TagValueResponse {
	    value?: tagging.Value;
	    error?: APIError;
	
	    static createFrom(source: any = {}) {
	        return new TagValueResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.value = this.convertValues(source["value"], tagging.Value);
	        this.error = this.convertValues(source["error"], APIError);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

}

export namespace library {
	
	export class ArchiveDetails {
	    originalFilename: string;
	    detectedFormat: string;
	    sha256: string;
	    byteSize: number;
	    verification: string;
	
	    static createFrom(source: any = {}) {
	        return new ArchiveDetails(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.originalFilename = source["originalFilename"];
	        this.detectedFormat = source["detectedFormat"];
	        this.sha256 = source["sha256"];
	        this.byteSize = source["byteSize"];
	        this.verification = source["verification"];
	    }
	}
	export class BooleanFilter {
	    definitionId: number;
	    state: string;
	
	    static createFrom(source: any = {}) {
	        return new BooleanFilter(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.definitionId = source["definitionId"];
	        this.state = source["state"];
	    }
	}
	export class ChoiceFilter {
	    definitionId: number;
	    valueIds: number[];
	
	    static createFrom(source: any = {}) {
	        return new ChoiceFilter(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.definitionId = source["definitionId"];
	        this.valueIds = source["valueIds"];
	    }
	}
	export class DisplaySources {
	    title: string;
	    description: string;
	    author: string;
	    artwork: string;
	
	    static createFrom(source: any = {}) {
	        return new DisplaySources(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.title = source["title"];
	        this.description = source["description"];
	        this.author = source["author"];
	        this.artwork = source["artwork"];
	    }
	}
	export class ListRequest {
	    page: number;
	    pageSize: number;
	    sort: string;
	
	    static createFrom(source: any = {}) {
	        return new ListRequest(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.page = source["page"];
	        this.pageSize = source["pageSize"];
	        this.sort = source["sort"];
	    }
	}
	export class StorySummary {
	    id: number;
	    uuid: string;
	    title: string;
	    description?: string;
	    author?: string;
	    artworkId?: string;
	    sources: DisplaySources;
	    detectedFormat: string;
	    compatibility: string;
	    compatibilityReason?: string;
	    byteSize: number;
	    importedAt: string;
	
	    static createFrom(source: any = {}) {
	        return new StorySummary(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.uuid = source["uuid"];
	        this.title = source["title"];
	        this.description = source["description"];
	        this.author = source["author"];
	        this.artworkId = source["artworkId"];
	        this.sources = this.convertValues(source["sources"], DisplaySources);
	        this.detectedFormat = source["detectedFormat"];
	        this.compatibility = source["compatibility"];
	        this.compatibilityReason = source["compatibilityReason"];
	        this.byteSize = source["byteSize"];
	        this.importedAt = source["importedAt"];
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class Page {
	    stories: StorySummary[];
	    page: number;
	    pageSize: number;
	    totalItems: number;
	    totalPages: number;
	    sort: string;
	
	    static createFrom(source: any = {}) {
	        return new Page(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.stories = this.convertValues(source["stories"], StorySummary);
	        this.page = source["page"];
	        this.pageSize = source["pageSize"];
	        this.totalItems = source["totalItems"];
	        this.totalPages = source["totalPages"];
	        this.sort = source["sort"];
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class StoryDetail {
	    story: StorySummary;
	    archive: ArchiveDetails;
	
	    static createFrom(source: any = {}) {
	        return new StoryDetail(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.story = this.convertValues(source["story"], StorySummary);
	        this.archive = this.convertValues(source["archive"], ArchiveDetails);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class StoryLibraryQuery {
	    name: string;
	    booleanFilters: BooleanFilter[];
	    choiceFilters: ChoiceFilter[];
	    page: number;
	    pageSize: number;
	    sort: string;
	
	    static createFrom(source: any = {}) {
	        return new StoryLibraryQuery(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.booleanFilters = this.convertValues(source["booleanFilters"], BooleanFilter);
	        this.choiceFilters = this.convertValues(source["choiceFilters"], ChoiceFilter);
	        this.page = source["page"];
	        this.pageSize = source["pageSize"];
	        this.sort = source["sort"];
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

}

export namespace metadata {
	
	export class CatalogStatus {
	    state: string;
	    locale: string;
	    matchedStoryCount: number;
	    fetchedAt?: string;
	    activatedAt?: string;
	    lastAttemptStatus?: string;
	    lastAttemptAt?: string;
	    errorCode?: string;
	    errorMessage?: string;
	
	    static createFrom(source: any = {}) {
	        return new CatalogStatus(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.state = source["state"];
	        this.locale = source["locale"];
	        this.matchedStoryCount = source["matchedStoryCount"];
	        this.fetchedAt = source["fetchedAt"];
	        this.activatedAt = source["activatedAt"];
	        this.lastAttemptStatus = source["lastAttemptStatus"];
	        this.lastAttemptAt = source["lastAttemptAt"];
	        this.errorCode = source["errorCode"];
	        this.errorMessage = source["errorMessage"];
	    }
	}

}

export namespace operations {
	
	export class ItemSnapshot {
	    id: number;
	    storyId?: number;
	    sourceName: string;
	    status: string;
	    outcomeCode?: string;
	    outcomeMessage?: string;
	    completedBytes: number;
	    totalBytes: number;
	
	    static createFrom(source: any = {}) {
	        return new ItemSnapshot(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.storyId = source["storyId"];
	        this.sourceName = source["sourceName"];
	        this.status = source["status"];
	        this.outcomeCode = source["outcomeCode"];
	        this.outcomeMessage = source["outcomeMessage"];
	        this.completedBytes = source["completedBytes"];
	        this.totalBytes = source["totalBytes"];
	    }
	}
	export class Snapshot {
	    id: string;
	    kind: string;
	    status: string;
	    completedItems: number;
	    totalItems: number;
	    cancelRequested: boolean;
	    errorCode?: string;
	    errorMessage?: string;
	    createdAt: string;
	    startedAt?: string;
	    finishedAt?: string;
	    items: ItemSnapshot[];
	
	    static createFrom(source: any = {}) {
	        return new Snapshot(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.kind = source["kind"];
	        this.status = source["status"];
	        this.completedItems = source["completedItems"];
	        this.totalItems = source["totalItems"];
	        this.cancelRequested = source["cancelRequested"];
	        this.errorCode = source["errorCode"];
	        this.errorMessage = source["errorMessage"];
	        this.createdAt = source["createdAt"];
	        this.startedAt = source["startedAt"];
	        this.finishedAt = source["finishedAt"];
	        this.items = this.convertValues(source["items"], ItemSnapshot);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

}

export namespace removal {
	
	export class Result {
	    storyId: number;
	    uuid: string;
	
	    static createFrom(source: any = {}) {
	        return new Result(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.storyId = source["storyId"];
	        this.uuid = source["uuid"];
	    }
	}

}

export namespace tagging {
	
	export class AssignmentResult {
	    requestedStories: number;
	    changedStories: number;
	    assignmentsAdded: number;
	    assignmentsRemoved: number;
	
	    static createFrom(source: any = {}) {
	        return new AssignmentResult(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.requestedStories = source["requestedStories"];
	        this.changedStories = source["changedStories"];
	        this.assignmentsAdded = source["assignmentsAdded"];
	        this.assignmentsRemoved = source["assignmentsRemoved"];
	    }
	}
	export class ValueAssignmentState {
	    valueId: number;
	    assignedStories: number;
	
	    static createFrom(source: any = {}) {
	        return new ValueAssignmentState(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.valueId = source["valueId"];
	        this.assignedStories = source["assignedStories"];
	    }
	}
	export class DefinitionAssignmentState {
	    definitionId: number;
	    assignedStories: number;
	    values: ValueAssignmentState[];
	
	    static createFrom(source: any = {}) {
	        return new DefinitionAssignmentState(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.definitionId = source["definitionId"];
	        this.assignedStories = source["assignedStories"];
	        this.values = this.convertValues(source["values"], ValueAssignmentState);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class Value {
	    id: number;
	    definitionId: number;
	    key: string;
	    normalizedKey: string;
	    label: string;
	    position: number;
	
	    static createFrom(source: any = {}) {
	        return new Value(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.definitionId = source["definitionId"];
	        this.key = source["key"];
	        this.normalizedKey = source["normalizedKey"];
	        this.label = source["label"];
	        this.position = source["position"];
	    }
	}
	export class DefinitionWithValues {
	    id: number;
	    key: string;
	    normalizedKey: string;
	    label: string;
	    color: string;
	    kind: string;
	    source: string;
	    presentation: string;
	    position: number;
	    protected: boolean;
	    values: Value[];
	
	    static createFrom(source: any = {}) {
	        return new DefinitionWithValues(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.key = source["key"];
	        this.normalizedKey = source["normalizedKey"];
	        this.label = source["label"];
	        this.color = source["color"];
	        this.kind = source["kind"];
	        this.source = source["source"];
	        this.presentation = source["presentation"];
	        this.position = source["position"];
	        this.protected = source["protected"];
	        this.values = this.convertValues(source["values"], Value);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class Catalog {
	    definitions: DefinitionWithValues[];
	
	    static createFrom(source: any = {}) {
	        return new Catalog(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.definitions = this.convertValues(source["definitions"], DefinitionWithValues);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class AssignmentWorkspace {
	    catalog: Catalog;
	    requestedStories: number;
	    states: DefinitionAssignmentState[];
	
	    static createFrom(source: any = {}) {
	        return new AssignmentWorkspace(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.catalog = this.convertValues(source["catalog"], Catalog);
	        this.requestedStories = source["requestedStories"];
	        this.states = this.convertValues(source["states"], DefinitionAssignmentState);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	
	export class CreateDefinition {
	    key: string;
	    label: string;
	    color: string;
	    kind: string;
	
	    static createFrom(source: any = {}) {
	        return new CreateDefinition(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.key = source["key"];
	        this.label = source["label"];
	        this.color = source["color"];
	        this.kind = source["kind"];
	    }
	}
	export class CreateValue {
	    definitionId: number;
	    key: string;
	    label: string;
	
	    static createFrom(source: any = {}) {
	        return new CreateValue(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.definitionId = source["definitionId"];
	        this.key = source["key"];
	        this.label = source["label"];
	    }
	}
	export class Definition {
	    id: number;
	    key: string;
	    normalizedKey: string;
	    label: string;
	    color: string;
	    kind: string;
	    source: string;
	    presentation: string;
	    position: number;
	    protected: boolean;
	
	    static createFrom(source: any = {}) {
	        return new Definition(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.key = source["key"];
	        this.normalizedKey = source["normalizedKey"];
	        this.label = source["label"];
	        this.color = source["color"];
	        this.kind = source["kind"];
	        this.source = source["source"];
	        this.presentation = source["presentation"];
	        this.position = source["position"];
	        this.protected = source["protected"];
	    }
	}
	
	export class DefinitionDeletionPlan {
	    definition: Definition;
	    valueCount: number;
	    assignmentCount: number;
	    affectedShelfCount: number;
	
	    static createFrom(source: any = {}) {
	        return new DefinitionDeletionPlan(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.definition = this.convertValues(source["definition"], Definition);
	        this.valueCount = source["valueCount"];
	        this.assignmentCount = source["assignmentCount"];
	        this.affectedShelfCount = source["affectedShelfCount"];
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	
	
	
	export class ValueDeletionPlan {
	    value: Value;
	    assignmentCount: number;
	    affectedShelfCount: number;
	
	    static createFrom(source: any = {}) {
	        return new ValueDeletionPlan(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.value = this.convertValues(source["value"], Value);
	        this.assignmentCount = source["assignmentCount"];
	        this.affectedShelfCount = source["affectedShelfCount"];
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

}
