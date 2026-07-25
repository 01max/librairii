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

