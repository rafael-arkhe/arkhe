package sbi

// models.go defines the shared 3GPP data types used across the SBI clients.

// PlmnId identifies a public land mobile network (TS 29.571).
type PlmnId struct {
	Mcc string `json:"mcc"`
	Mnc string `json:"mnc"`
}

// AccessAndMobilitySubscriptionData is the Nudm_SDM GetAMData response.
type AccessAndMobilitySubscriptionData struct {
	Gpsis []string `json:"gpsis"`
	SubscribedUeAmbr struct {
		Uplink   string `json:"uplink"`
		Downlink string `json:"downlink"`
	} `json:"subscribedUeAmbr"`
	Nssai []struct {
		Sst int    `json:"sst"`
		Sd  string `json:"sd,omitempty"`
	} `json:"nssai"`
	RatRestrictions []struct {
		RatType string `json:"ratType"`
		Allowed bool   `json:"allowed"`
	} `json:"ratRestrictions"`
}

// NamfLocResp models the ProvidePositioningInfo response (TS 29.518).
type NamfLocResp struct {
	LocationInfo struct {
		CellId   string  `json:"cellId"`
		Tac      string  `json:"tac"`
		PlmnId   PlmnId  `json:"plmnId"`
		Age      int     `json:"ageOfLocationInfo"`
		Accuracy string  `json:"accuracy,omitempty"`
	} `json:"locationInfo"`
}
